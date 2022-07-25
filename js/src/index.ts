// This is a simplified database for the browser. No history is stored.
import Map2 from "map2"

export const ROOT: Version = ['ROOT', 0]

export type Version = [agent: string, seq: number]

export type Primitive = null
  | boolean
  | string
  | number
  | Primitive[]
  | {[k: string]: Primitive}

export type CreateValue = {type: 'primitive', val: Primitive}
  | {type: 'crdt', crdtKind: 'map' | 'set' | 'register'}

type RegisterValue = {type: 'primitive', val: Primitive}
  | {type: 'crdt', id: Version}

type MVRegister = [Version, RegisterValue][]

type CRDTInfo = {
  type: 'map',
  registers: {[k: string]: MVRegister},
} | {
  type: 'set',
  values: Map2<string, number, RegisterValue>,
} | {
  type: 'register',
  value: MVRegister,
}

interface DBState {
  version: Version[],
  crdts: Map2<string, number, CRDTInfo>
}

export type Action =
{ type: 'map', key: string, localParents: Version[], val: CreateValue }
| { type: 'registerSet', localParents: Version[], val: CreateValue }
| { type: 'setInsert', val: CreateValue }
| { type: 'setDelete', target: Version }

export interface Operation {
  id: Version,
  globalParents: Version[],
  crdtId: Version,
  action: Action,
}

const versionEq = ([a1, s1]: Version, [a2, s2]: Version) => (a1 === a2 && s1 === s2)
const versionCmp = ([a1, s1]: Version, [a2, s2]: Version) => (
  a1 < a2 ? 1
    : a1 > a2 ? -1
    : s1 - s2
)

export const advanceFrontier = (frontier: Version[], version: Version, parents: Version[]): Version[] => {
  const f = frontier.filter(v => !parents.some(v2 => versionEq(v, v2)))
  f.push(version)
  return f.sort(versionCmp)
}

export function createDb(): DBState {
  const db: DBState = {
    version: [],
    crdts: new Map2(),
  }

  db.crdts.set(ROOT[0], ROOT[1], {
    type: "map",
    registers: {}
  })

  return db
}

function removeRecursive(state: DBState, value: RegisterValue) {
  if (value.type !== 'crdt') return

  const crdt = state.crdts.get(value.id[0], value.id[1])
  if (crdt == null) return

  switch (crdt.type) {
    case 'map':
      for (const k in crdt.registers) {
        const reg = crdt.registers[k]
        for (const [version, value] of reg) {
          removeRecursive(state, value)
        }
      }
      break
    case 'register':
      for (const [version, value] of crdt.value) {
        removeRecursive(state, value)
      }
      break
    case 'set':
      for (const [agent, seq, value] of crdt.values) {
        removeRecursive(state, value)
      }
      break
    default: throw Error('Unknown CRDT type!?')
  }

  state.crdts.delete(value.id[0], value.id[1])
}

export function localRegisterSet(state: DBState, id: Version, regId: Version, val: CreateValue): Operation {
  const crdt = state.crdts.get(regId[0], regId[1])
  if (crdt == null || crdt.type !== 'register') throw Error('invalid CRDT')

  const localParents = crdt.value.map(([version]) => version)
  const op: Operation = {
    id,
    crdtId: regId,
    globalParents: state.version,
    action: { type: 'registerSet', localParents, val }
  }
  // TODO: Inline this?
  applyRemoteOp(state, op)
  return op
}

export function localMapInsert(state: DBState, id: Version, mapId: Version, key: string, val: CreateValue): Operation {
  const crdt = state.crdts.get(mapId[0], mapId[1])
  if (crdt == null || crdt.type !== 'map') throw Error('invalid CRDT')

  const localParents = (crdt.registers[key] ?? []).map(([version]) => version)
  const op: Operation = {
    id,
    crdtId: mapId,
    globalParents: state.version,
    action: { type: 'map', localParents, key, val }
  }
  // TODO: Could easily inline this - which would mean more code but higher performance.
  applyRemoteOp(state, op)
  return op
}

export function localSetInsert(state: DBState, id: Version, setId: Version, val: CreateValue): Operation {
  const crdt = state.crdts.get(setId[0], setId[1])
  if (crdt == null || crdt.type !== 'set') throw Error('invalid CRDT')

  const op: Operation = {
    id,
    crdtId: setId,
    globalParents: state.version,
    action: { type: 'setInsert', val }
  }
  // TODO: Inline this?
  applyRemoteOp(state, op)
  return op
}

export function localSetDelete(state: DBState, id: Version, setId: Version, target: Version): Operation | null {
  const crdt = state.crdts.get(setId[0], setId[1])
  if (crdt == null || crdt.type !== 'set') throw Error('invalid CRDT')

  let oldVal = crdt.values.get(target[0], target[1])
  if (oldVal != null) {
    removeRecursive(state, oldVal)
    crdt.values.delete(target[0], target[1])

    return {
      id,
      crdtId: setId,
      globalParents: state.version,
      action: { type: 'setDelete', target }
    }
  } else { return null } // Already deleted.
}


const errExpr = (str: string): never => { throw Error(str) }

function createCRDT(state: DBState, id: Version, type: 'map' | 'set' | 'register') {
  if (state.crdts.has(id[0], id[1])) {
    throw Error('CRDT already exists !?')
  }

  const crdtInfo: CRDTInfo = type === 'map' ? {
    type: "map",
    registers: {},
  } : type === 'register' ? {
    type: 'register',
    value: [],
  } : type === 'set' ? {
    type: 'set',
    values: new Map2,
  } : errExpr('Invalid CRDT type')

  state.crdts.set(id[0], id[1], crdtInfo)
}

function mergeRegister(state: DBState, oldPairs: MVRegister, localParents: Version[], newVersion: Version, newVal: CreateValue): MVRegister {
  const newPairs: MVRegister = []
  for (const [version, value] of oldPairs) {
    // Each item is either retained or removed.
    if (localParents.some(v2 => versionEq(version, v2))) {
      // The item was named in parents. Remove it.
      console.log('removing', value)
      removeRecursive(state, value)
    } else {
      newPairs.push([version, value])
    }
  }

  let newValue: RegisterValue
  if (newVal.type === 'primitive') {
    newValue = newVal
  } else {
    // Create it.
    createCRDT(state, newVersion, newVal.crdtKind)
    newValue = {type: "crdt", id: newVersion}
  }

  newPairs.push([newVersion, newValue])
  newPairs.sort(([v1], [v2]) => versionCmp(v1, v2))

  return newPairs
}

export function applyRemoteOp(state: DBState, op: Operation) {
  state.version = advanceFrontier(state.version, op.id, op.globalParents)

  const crdt = state.crdts.get(op.crdtId[0], op.crdtId[1])
  if (crdt == null) {
    console.warn('CRDT has been deleted..')
    return
  }

  // Every map operation creates a new value, and removes 0-n other values.
  switch (op.action.type) {
    case 'registerSet': {
      if (crdt.type !== 'register') throw Error('Invalid operation type for target')
      const newPairs = mergeRegister(state, crdt.value, op.action.localParents, op.id, op.action.val)

      crdt.value = newPairs
      break
    }
    case 'map': {
      if (crdt.type !== 'map') throw Error('Invalid operation type for target')

      const oldPairs = crdt.registers[op.action.key] ?? []
      const newPairs = mergeRegister(state, oldPairs, op.action.localParents, op.id, op.action.val)

      crdt.registers[op.action.key] = newPairs
      break
    }
    case 'setInsert': case 'setDelete': { // Sets!
      if (crdt.type !== 'set') throw Error('Invalid operation type for target')

      // Set operations are comparatively much simpler, because insert
      // operations cannot be concurrent and multiple overlapping delete
      // operations are ignored.

      if (op.action.type == 'setInsert') {
        if (op.action.val.type === 'primitive') {
          crdt.values.set(op.id[0], op.id[1], op.action.val)
        } else {
          const activeValue = createCRDT(state, op.id, op.action.val.crdtKind)
          crdt.values.set(op.id[0], op.id[1], {type: "crdt", id: op.id})
        }
      } else {
        // Delete!
        let oldVal = crdt.values.get(op.action.target[0], op.action.target[1])
        if (oldVal != null) {
          removeRecursive(state, oldVal)
          crdt.values.delete(op.action.target[0], op.action.target[1])
        }
      }

      break
    }

    default: throw Error('Invalid action type')
  }
}

export type DBValue = null
  | boolean
  | string
  | number
  | DBValue[]
  | {[k: string]: DBValue} // Map
  | Map2<string, number, DBValue> // Set.

const registerToVal = (state: DBState, r: RegisterValue): DBValue => (
  (r.type === 'primitive')
    ? r.val
    : get(state, r.id) // Recurse!
)

export function get(state: DBState, crdtId: Version = ROOT): DBValue {
  const crdt = state.crdts.get(crdtId[0], crdtId[1])
  if (crdt == null) { return null }

  switch (crdt.type) {
    case 'register': {
      // When there's a tie, the active value is based on the order in pairs.
      const activePair = crdt.value[0][1]
      return registerToVal(state, activePair)
    }
    case 'map': {
      const result: {[k: string]: DBValue} = {}
      for (const k in crdt.registers) {
        const activePair = crdt.registers[k][0][1]
        result[k] = registerToVal(state, activePair)
      }
      return result
    }
    case 'set': {
      const result = new Map2<string, number, DBValue>()

      for (const [agent, seq, value] of crdt.values) {
        result.set(agent, seq, registerToVal(state, value))
      }

      return result
    }
    default: throw Error('Invalid CRDT type in DB')
  }
}