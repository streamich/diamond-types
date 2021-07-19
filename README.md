# Diamond Types

> Note: This code is currently a work in progress. Do not use it for anything serious yet. It is missing important features and it contains bugs.

This repository contains a high performance rust list CRDT. This is a
special data type which supports concurrent editing of lists or strings
(text documents) by multiple users in a P2P network without needing a
centralized server.

For much more detail about how this library works, see the talk I gave
on this library at a recent [braid user
meetings](https://braid.org/meeting-14).

For now there is only a list implementation here. At some point I'd like
to add other data structures (objects, tuples, etc). (Hence the name.)

This project was initially created as a prototype to see how fast a well
optimized CRDT could be made to go. The answer is really fast - faster
than other similar libraries. This library is currently in the process
of being expanded into a fast, feature rich CRDT in its own right.

This library is also designed to be interoperable with positional
updates, which will allow simple peers to interact with the data
structure via an OT system. (WIP)


## Internals

Each client / device has a unique ID. Each character typed or deleted on
each device is assigned an incrementing sequence number (starting at 0).
Each character in the document can thus be uniquely identified by the
tuple of `(client ID, sequence number)`. This allows any location in the
document to be uniquely named.

The internal data structures are designed to optimize two main operations:

- Text edit to CRDT operation (Eg, "user A inserts at position 100" -> "user A
  seq 1000 inserts at (B, 50)")
- CRDT operation to text edit ("user A
  seq 1000 inserts at (B, 50)" -> "insert at document position 100")

Actually inserting text into a rope (or something) is reasonably easy, and
[ropey](https://github.com/cessen/ropey/) and
[xi-rope](https://crates.io/crates/xi-rope) both seem very [high
performance](https://home.seph.codes/public/c3/ins_random/report/index.html). So
in this library I'm worrying about the P2P operation boundary.

Internally, each client ID is mapped to a local *"order"* integer. These integers are
never sent over the wire, so it doesn't matter if they aren't common between
peers.

Then we have two main data structures internally:

- A modified B-Tree of the entire document in sequence order. Entries are run
  encoded - which is to say, sequences of (client: 1, seq: 100), (client: 1,
  seq: 101), (client: 1, seq: 102), ... are collapsed into entries of (client:
  1, seq: 100, len: 5). Deleted characters are marked with a negative length.
  The tree's internal nodes store subtree sizes, so we can map from an entry in
  the tree to a document position (and back again) in O(log n) time.
- Per client operation lists. These are simply arrays of pointers mapping each
  sequential sequence number to the leaf node in the tree containing that item.
  These lists could also be run-length encoded - which would save memory, but
  then we'd need to binary search to find any given location instead of doing a
  simple array lookup.


## Optimizations to perform

- [ ] Cache the last 


## Progress

- [x] Basic btree structure implemented
- [x] Check function to verify structure integrity
- [x] Doc Location -> CRDT location name
- [x] CRDT location -> Doc location
- [x] Insert text
- [x] Remove text
- [ ] Cleanup
- [ ] Cache last location for each client
- [ ] Tests
- [ ] Benchmarks
