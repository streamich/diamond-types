//! This file contains some wrappers to interact with operations publicly.

use crate::causalgraph::remote_ids::{RemoteFrontier, RemoteVersion, RemoteVersionOwned};
use crate::{CollectionOp, CreateValue, LV, Op, OpContents, OpLog, ROOT_CRDT_ID, ROOT_CRDT_ID_AV};
use smartstring::alias::String as SmartString;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use crate::causalgraph::agent_span::AgentVersion;
use crate::rle::KVPair;
use crate::simpledb::SimpleDatabase;

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ExtOpContents<'a> {
    RegisterSet(CreateValue),
    MapSet(SmartString, CreateValue),
    MapDelete(SmartString),
    CollectionInsert(CreateValue),
    #[cfg_attr(feature = "serde", serde(borrow))]
    CollectionRemove(RemoteVersion<'a>),
    // Text(ListOpMetrics),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ExtOp<'a> {
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub target: RemoteVersion<'a>,
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub parents: RemoteFrontier<'a>,
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub version: RemoteVersion<'a>, // Start version big ops
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub contents: ExtOpContents<'a>
}

impl OpLog {
    fn target_to_rv(&self, target: LV) -> RemoteVersion<'_> {
        if target == ROOT_CRDT_ID {
            RemoteVersion("ROOT", 0)
        } else {
            self.cg.local_to_remote_version(target)
        }
    }

    fn op_contents_to_ext_op(&self, op: OpContents) -> ExtOpContents<'_> {
        match op {
            OpContents::RegisterSet(val) => ExtOpContents::RegisterSet(val),
            OpContents::MapSet(key, val) => ExtOpContents::MapSet(key, val),
            OpContents::MapDelete(key) => ExtOpContents::MapDelete(key),
            OpContents::Collection(CollectionOp::Insert(val)) => ExtOpContents::CollectionInsert(val),
            OpContents::Collection(CollectionOp::Remove(lv)) => ExtOpContents::CollectionRemove(
                self.cg.local_to_remote_version(lv)
            ),
            OpContents::Text(_) => { unimplemented!() }
        }
    }

    pub fn ext_ops_since(&self, v: &[LV]) -> Vec<ExtOp> {
        let mut result = vec![];

        for walk in self.cg.parents.optimized_txns_between(v, self.version.as_ref()) {
            for KVPair(lv, op) in self.uncommitted_ops.ops.iter_range_packed_ctx(walk.consume, &self.uncommitted_ops.list_ctx) {


                result.push(ExtOp {
                    target: self.target_to_rv(op.target_id),
                    parents: self.cg.local_to_remote_frontier(self.cg.parents.parents_at_time(lv).as_ref()),
                    version: self.cg.local_to_remote_version(lv),
                    contents: self.op_contents_to_ext_op(op.contents)
                });

                // result.push(ops.1);
            }
        }

        result
    }

    fn ext_contents_to_local(&self, ext: ExtOpContents) -> OpContents {
        match ext {
            ExtOpContents::RegisterSet(val) => OpContents::RegisterSet(val),
            ExtOpContents::MapSet(key, val) => OpContents::MapSet(key, val),
            ExtOpContents::MapDelete(key) => OpContents::MapDelete(key),
            ExtOpContents::CollectionInsert(val) => OpContents::Collection(CollectionOp::Insert(val)),
            ExtOpContents::CollectionRemove(rv) => OpContents::Collection(CollectionOp::Remove(
                self.cg.remote_to_local_version(rv)
            ))
        }
    }

    fn target_rv_to_av(&self, target: RemoteVersion) -> AgentVersion {
        if target.0 == "ROOT" {
            ROOT_CRDT_ID_AV
        } else {
            self.cg.remote_to_agent_version_known(target)
        }
    }

    pub fn merge_ext_ops(&mut self, ops: Vec<ExtOp>) {
        for op in ops {
            let ExtOp {
                target, parents, version, contents
            } = op;

            let parents_local = self.cg.remote_to_local_frontier(parents.into_iter());
            // let target_local = self.cg.remote_to_local_version(target);
            let version_local = self.cg.remote_to_agent_version_unknown(version);
            let target_local = self.target_rv_to_av(target);
            let contents = self.ext_contents_to_local(contents);

            self.push_remote_op(parents_local.as_ref(), version_local.into(), target_local, contents);
        }
    }
}

impl SimpleDatabase {
    pub fn merge_ext_ops(&mut self, ops: Vec<ExtOp>) {
        for op in ops {
            let ExtOp {
                target, parents, version, contents
            } = op;

            let parents_local = self.oplog.cg.remote_to_local_frontier(parents.into_iter());
            // let target_local = self.cg.remote_to_local_version(target);
            let version_local = self.oplog.cg.remote_to_agent_version_unknown(version);
            let target_local = self.oplog.target_rv_to_av(target);
            let contents = self.oplog.ext_contents_to_local(contents);

            self.apply_remote_op(parents_local.as_ref(), version_local.into(), target_local, contents);
            // self.push_remote_op(parents_local.as_ref(), version_local.into(), target_local, contents);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{CRDTKind, OpLog};
    use crate::ROOT_CRDT_ID;
    use crate::simpledb::SimpleDatabase;
    use crate::Primitive::*;
    use crate::CreateValue::*;
    use crate::CreateValue::*;

    #[test]
    fn external_ops_merge() {
        let mut db = SimpleDatabase::new_mem();
        let seph = db.get_or_create_agent_id("seph");
        db.map_set(seph, ROOT_CRDT_ID, "name", Primitive(Str("seph".into())));

        let inner = db.map_set(seph, ROOT_CRDT_ID, "facts", NewCRDT(CRDTKind::Map));
        db.map_set(seph, inner, "cool", Primitive(I64(1)));

        let inner_set = db.map_set(seph, ROOT_CRDT_ID, "set stuff", NewCRDT(CRDTKind::Collection));
        let inner_map = db.collection_insert(seph, inner_set, NewCRDT(CRDTKind::Map));
        db.map_set(seph, inner_map, "whoa", Primitive(I64(3214)));

        let ops_ext = db.oplog.ext_ops_since(&[]);
        // println!("{}", serde_json::to_string(&ops_ext).unwrap());
        // dbg!(&ops_ext);

        // let mut oplog2 = OpLog::new_mem();
        // oplog2.merge_ext_ops(ops_ext);
        //
        // dbg!(&db.oplog);
        // dbg!(&oplog2);

        let mut db2 = SimpleDatabase::new_mem();
        db2.merge_ext_ops(ops_ext);
        assert_eq!(db.get_recursive(), db2.get_recursive());
        // dbg!(db2.get_recursive());

        // println!("{}", serde_json::to_string(&db2.get_recursive()).unwrap());
    }
}