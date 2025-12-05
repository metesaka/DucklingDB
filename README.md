# DucklingDB

I am a PhD student and main research area is databases. This is my fun project to develop a database from scratch using Rust.

This database project has nothing to do with [DuckDB](https://duckdb.org/) (which is awesome and everyone should use it), I just like ducks and I found it funny to name it as "Small DuckDB". 

The original system implementation is inspired by legendary [System R](https://dl.acm.org/doi/10.1145/320455.320457) paper. 

After completing the full working prototype, I'll move on with developing improvements that was also introduced to database literature over the past 50 years after the introduction of system R.


## Implementation Progress

Todo list created by chatgpt after some point, subject to change

- [x] Disk Manager
- [x] Buffer Manager
- [x] Slotted page implementation
- [x] Tuple Manager (Heap File)
- [ ] Tuple Manager (schema and data types)
    - [ ] Row layout with fixed and varlen fields
    - [ ] Basic types: i32, i64, f64, bool, varchar, bytes
    - [ ] Null bitmap and simple tuple descriptor
- [ ] Encode and decode helpers and column projection
- [ ] Segment and table management
    - [ ] Segment abstraction and page directory
    - [ ] Table metadata: table id, root page, page count
    - [ ] Free space map per table and startup rebuild
    - [ ] Table API: insert, get, update, delete, scan

- [ ] Indexes
    - [ ] B plus tree page format: internal and leaf
    - [ ] Search and point lookup
    - [ ] Insert with split and merge
    - [ ] Range scan via leaf links
    - [ ] Tie index entries to record ids
    - [ ] Unique index check

- [ ] Concurrency control
    - [ ] Page latches for physical safety
    - [ ] Transaction manager and ids
    - [ ] Lock manager with shared and exclusive locks
    - [ ] Two phase locking and lock release on commit

- [ ] Query execution
- [ ] SQL parser 
- [ ] Testing and tooling
- [ ] Query optimization
- [ ] CLI
- [ ] ...

