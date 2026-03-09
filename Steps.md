# DucklingDB: System R Implementation Plan

A step-by-step plan to implement System R in Rust, organized into ~1-hour tasks.
Each task is self-contained with clear inputs, outputs, and acceptance criteria.

Rust Book references point to: https://doc.rust-lang.org/book/

---

## Phase 0: Rust Foundations & Project Hygiene (5 hours)

These tasks build the Rust skills needed for the entire project and clean up the existing codebase.

### Task 0.1 — Rust: Ownership, Borrowing, and Lifetimes (1 hr)
**Read:** Rust Book Ch. 4 (Understanding Ownership) — all three sections.
**Exercises:**
- Write a small program that moves a `String` into a function and observe the compiler error when you try to use it after.
- Write a function that takes `&str` and one that takes `&mut String`. Understand when each is appropriate.
- Relate this to DucklingDB: the buffer manager hands out `&mut [u8]` page references — why does this matter?
**Done when:** You can explain to yourself why `Arc<Mutex<>>` is used in `buffer_manager.rs` and what would happen without it.

### Task 0.2 — Rust: Error Handling (1 hr)
**Read:** Rust Book Ch. 9 (Error Handling) — all sections.
**Exercises:**
- Understand `Result<T, E>`, the `?` operator, and `thiserror`/`anyhow` crates.
- Define a `DucklingError` enum in a new file `src/error.rs`:
  ```rust
  pub enum DucklingError {
      IoError(std::io::Error),
      BufferPoolFull,
      PageOverflow,
      SlotNotFound,
      TupleNotFound,
      InvalidPageId,
  }
  ```
- Implement `From<std::io::Error>` for it.
- Add `pub type Result<T> = std::result::Result<T, DucklingError>;`
**Done when:** `cargo build` succeeds with the new error module added to `lib.rs`.

### Task 0.3 — Rust: Traits and Generics (1 hr)
**Read:** Rust Book Ch. 10 (Generic Types, Traits, Lifetimes) — sections 10.1 and 10.2.
**Exercises:**
- Define a `Pager` trait with methods `read_page`, `write_page`, `allocate_page`.
- Make `DiskManager` implement this trait.
- Understand why traits are like interfaces — this will be critical for testing (mock disk managers).
**Done when:** You can call `DiskManager` methods through a `&dyn Pager` reference.

### Task 0.4 — Refactor: Proper Error Handling in Existing Code (1 hr)
**Goal:** Replace all `.unwrap()` calls in the existing codebase with proper `Result` returns.
**Steps:**
1. Change `DiskManager` methods to return `Result<T>`.
2. Change `BufferPoolManager` methods to return `Result<T>`.
3. Change `SlottedPage` methods to return `Result<T>` where appropriate (keep `Option` for `insert` since "page full" is expected).
4. Change `HeapFile` methods to return `Result<T>`.
5. Fix compiler warnings (unused variables, dead code).
**Done when:** `cargo build` has zero warnings. All functions return `Result` or `Option` instead of panicking.

### Task 0.5 — Refactor: Convert main.rs Tests to Proper Unit/Integration Tests (1 hr)
**Read:** Rust Book Ch. 11 (Writing Automated Tests) — all sections.
**Steps:**
1. Create `tests/integration_test.rs` for end-to-end tests.
2. Move the manual test code from `main.rs` into proper `#[test]` functions.
3. Add unit tests for `SlottedPage`: insert, read, delete, update, compact, iterator.
4. Add unit tests for `HeapFile`: insert, read across multiple pages.
5. Make `main.rs` a clean entry point (just a placeholder `println!` for now).
**Done when:** `cargo test` runs all tests and they pass. `main.rs` is minimal.

---

## Phase 1: Tuple Layout & Schema (5 hours)

System R stores n-ary relations where each tuple has fixed and variable-length fields. This phase builds the schema and tuple encoding layer — the foundation everything else depends on.

### Task 1.1 — Rust: Enums, Pattern Matching, and Structs (1 hr)
**Read:** Rust Book Ch. 5 (Structs) and Ch. 6 (Enums and Pattern Matching).
**Exercises:**
- Define the core data types as a Rust enum:
  ```rust
  pub enum DataType {
      Integer,      // i32, 4 bytes
      BigInt,       // i64, 8 bytes
      Float,        // f64, 8 bytes
      Boolean,      // bool, 1 byte
      Varchar(u16), // variable length, max length
  }
  ```
- Define a `Value` enum that holds actual data:
  ```rust
  pub enum Value {
      Integer(i32),
      BigInt(i64),
      Float(f64),
      Boolean(bool),
      Varchar(String),
      Null,
  }
  ```
- Practice `match` on `Value` to extract data. Understand exhaustive matching.
**Done when:** You can create `Value` instances and match on them without compiler warnings.

### Task 1.2 — Schema Definition (1 hr)
**Goal:** Create `src/schema.rs` with column and schema definitions.
**Implement:**
```rust
pub struct Column {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
}

pub struct Schema {
    pub columns: Vec<Column>,
}
```
**Methods on `Schema`:**
- `new(columns: Vec<Column>) -> Schema`
- `num_columns() -> usize`
- `find_column(name: &str) -> Option<usize>` — return column index
- `fixed_len() -> usize` — total bytes for all fixed-length fields
- `has_variable_fields() -> bool`
**Tests:** Create an EMP schema (EMPNO, NAME, DNO, JOB, SAL, MGR) from the paper and verify column lookups.
**Done when:** All tests pass and schema correctly reports fixed vs variable field sizes.

### Task 1.3 — Tuple Serialization: Encoding (1 hr)
**Goal:** Create `src/tuple.rs` — encode a `Vec<Value>` into a byte array that fits in a slotted page.
**Layout (per System R, Section 3 "Relations"):**
```
[Null bitmap (ceil(n_cols/8) bytes)][Fixed fields][Variable length offset array][Variable data]
```
- Null bitmap: 1 bit per column, 1 = null, 0 = present.
- Fixed fields: written in column order, only for non-null fixed-size columns.
- Variable-length offset array: one `u16` per varchar column, pointing to start of data within the variable section.
- Variable data: concatenated varchar bytes.

**Implement:**
- `pub fn encode(values: &[Value], schema: &Schema) -> Vec<u8>`
**Tests:**
- Encode a tuple with all fixed fields, decode manually to verify bytes.
- Encode a tuple with a Null value, verify the null bitmap.
- Encode a tuple with varchar fields, verify offsets are correct.
**Done when:** Encoding produces correct byte representations for mixed fixed/variable tuples.

### Task 1.4 — Tuple Serialization: Decoding (1 hr)
**Goal:** Decode a byte array back into `Vec<Value>` using the schema.
**Implement:**
- `pub fn decode(data: &[u8], schema: &Schema) -> Result<Vec<Value>>`
- `pub fn decode_field(data: &[u8], schema: &Schema, col_idx: usize) -> Result<Value>` — decode a single field without decoding the whole tuple (important for projections later).
**Tests:**
- Round-trip: encode then decode, verify values match.
- Decode individual fields from an encoded tuple.
- Null handling: encode null, decode null, verify `Value::Null`.
**Done when:** Full round-trip encode/decode works for all data types including nulls and varchars.

### Task 1.5 — Integrate Tuples with HeapFile (1 hr)
**Goal:** Create `src/table.rs` — a typed layer over `HeapFile` that understands schemas.
**Implement:**
```rust
pub struct Table {
    pub name: String,
    pub schema: Schema,
    pub heap_file: HeapFile,
}
```
**Methods:**
- `insert(&mut self, values: &[Value]) -> Result<TupleId>` — encode and insert.
- `read(&mut self, tid: TupleId) -> Result<Vec<Value>>` — read and decode.
- `scan(&mut self) -> Result<Vec<(TupleId, Vec<Value>)>>` — full table scan, iterate all pages and all tuples.
- `delete(&mut self, tid: TupleId) -> Result<()>`
**Tests:**
- Create an EMP table, insert 5 rows, scan and verify all 5 come back.
- Insert, delete, scan — verify deleted row is gone.
- Insert enough rows to span 2+ pages, scan all.
**Done when:** You can store and retrieve typed tuples through the Table API.

---

## Phase 2: System Catalog (3 hours)

System R maintains catalog relations describing all tables, columns, and indexes. The catalog is itself stored as relations — queried the same way as user data.

### Task 2.1 — Rust: Collections and Iterators (1 hr)
**Read:** Rust Book Ch. 8 (Common Collections) — Vectors, Strings, HashMaps.
**Read:** Rust Book Ch. 13.2-13.3 (Iterators).
**Exercises:**
- Practice `HashMap<String, T>` — you'll use this for catalog lookups.
- Chain iterator operations: `.filter().map().collect()` — this pattern will be everywhere in query execution.
- Understand `impl Iterator<Item = T>` — you'll return iterators from scans.
**Done when:** You can fluently use HashMap and iterator chains.

### Task 2.2 — Catalog Manager: Table Metadata (1 hr)
**Goal:** Create `src/catalog.rs` that tracks all tables in the system.
**System R stores these catalog relations (Section 2, "Data Definition Facilities"):**
- `SYSCOLUMNS`: one row per column of every table (column name, type, table it belongs to, ordinal position).
- `SYSTABLES`: one row per table (table name, number of columns, storage info).

**For now, implement an in-memory catalog:**
```rust
pub struct Catalog {
    tables: HashMap<String, TableInfo>,
}

pub struct TableInfo {
    pub name: String,
    pub schema: Schema,
    pub heap_file_id: u64, // or however you identify the heap file
}
```
**Methods:**
- `create_table(name: &str, columns: Vec<Column>) -> Result<()>`
- `drop_table(name: &str) -> Result<()>`
- `get_table(name: &str) -> Option<&TableInfo>`
- `list_tables() -> Vec<&str>`
**Done when:** You can create tables through the catalog and look them up by name.

### Task 2.3 — Database: Top-Level Coordinator (1 hr)
**Goal:** Create `src/database.rs` — the top-level struct that owns the disk manager, buffer pool, and catalog.
```rust
pub struct Database {
    disk_manager: DiskManager,
    buffer_pool: Arc<Mutex<BufferPoolManager>>,
    catalog: Catalog,
}
```
**Methods:**
- `open(path: &str) -> Result<Database>` — open or create a database file.
- `create_table(name: &str, columns: Vec<Column>) -> Result<()>`
- `get_table(name: &str) -> Result<Table>` — return a Table handle.
- `close(self) -> Result<()>` — flush all dirty pages.

**Integration test:**
```rust
let db = Database::open("test.db")?;
db.create_table("emp", vec![
    Column::new("empno", DataType::Integer, false),
    Column::new("name", DataType::Varchar(50), false),
    Column::new("sal", DataType::Integer, true),
])?;
let mut emp = db.get_table("emp")?;
emp.insert(&[Value::Integer(1), Value::Varchar("Alice".into()), Value::Integer(50000)])?;
```
**Done when:** End-to-end flow works — create database, create table, insert and read tuples.

---

## Phase 3: B+ Tree Index (8 hours)

System R calls indexes "images" — B-tree structures that provide associative and sequential access. This is the most complex data structure in the system.

### Task 3.1 — Rust: Smart Pointers and Interior Mutability (1 hr)
**Read:** Rust Book Ch. 15 (Smart Pointers) — Box, Rc, RefCell sections.
**Read:** Rust Book Ch. 16.3 (Shared-State Concurrency) — Mutex, Arc.
**Exercises:**
- Understand `Box<T>` for heap allocation — B+ tree nodes will use this.
- Understand `Arc<Mutex<T>>` — the buffer pool uses this pattern.
- Write a small tree structure using `Box` for child pointers.
**Done when:** You can build a simple binary tree with `Box<Node>` children.

### Task 3.2 — B+ Tree: Page Layout for Leaf and Internal Nodes (1 hr)
**Goal:** Create `src/btree/page.rs` — define the on-disk layout for B+ tree pages.
**System R (Section 3, "Images"):** Each index is a balanced hierarchic structure. Leaf nodes contain (sort-value, TID list). Internal nodes contain (sort-value, child-page-pointer).

**Leaf page layout:**
```
[PageType(1)][NumKeys(u16)][NextLeaf(u64)][PrevLeaf(u64)]
[Key1][TID1][Key2][TID2]...
```

**Internal page layout:**
```
[PageType(1)][NumKeys(u16)][LeftChild(u64)]
[Key1][RightChild1][Key2][RightChild2]...
```

**Implement:**
- `BTreePageType` enum: `Leaf`, `Internal`
- Helper functions to read/write keys and pointers from a page buffer.
- Support for `Integer` keys first (fixed 4-byte keys). Variable-length keys come later.
**Done when:** You can write keys/pointers to a page buffer and read them back correctly.

### Task 3.3 — B+ Tree: Search (Point Lookup) (1 hr)
**Goal:** Create `src/btree/mod.rs` — implement search from root to leaf.
**Algorithm:**
1. Start at root page.
2. If internal node: binary search keys, follow the appropriate child pointer.
3. If leaf node: binary search keys, return the TID if found.

**Implement:**
```rust
pub struct BTree {
    root_page_id: PageId,
    bpm: Arc<Mutex<BufferPoolManager>>,
}

impl BTree {
    pub fn search(&self, key: &Value) -> Result<Option<TupleId>>
}
```
**Test:** Manually construct a small B+ tree (1 root internal + 2 leaves) by writing pages directly, then search for existing and non-existing keys.
**Done when:** Point lookups work on a manually constructed tree.

### Task 3.4 — B+ Tree: Insertion Without Splits (1 hr)
**Goal:** Insert keys into leaf nodes that have space.
**Algorithm:**
1. Search for the correct leaf.
2. Insert the key in sorted order within the leaf.
3. Mark the page dirty.

**Implement:**
- `pub fn insert(&mut self, key: &Value, tid: TupleId) -> Result<()>`
- For now, assume the leaf has space (no splits).

**Test:**
- Create a B+ tree with a single empty leaf as root.
- Insert 5 keys, verify they're stored in sorted order.
- Search for each key after insertion.
**Done when:** Insertion into non-full leaves works and search finds all inserted keys.

### Task 3.5 — B+ Tree: Leaf Splits (1 hr)
**Goal:** Handle the case when a leaf node is full.
**Algorithm:**
1. When a leaf is full, allocate a new leaf page.
2. Move the upper half of keys to the new leaf.
3. Update sibling pointers (next/prev leaf).
4. Push the middle key up to the parent.
5. If there's no parent (root was a leaf), create a new root internal node.

**Implement:** Extend `insert()` to handle leaf splits. Return a "split info" struct up the call stack:
```rust
struct SplitResult {
    new_key: Value,      // key to push up
    new_page_id: PageId, // right sibling
}
```
**Test:**
- Set a small page size or small max keys per leaf (e.g., 4).
- Insert 5 keys to force a split.
- Verify both leaves have correct keys and the root internal node has the split key.
- Search for all 5 keys.
**Done when:** Leaf splits work and the tree remains searchable after splits.

### Task 3.6 — B+ Tree: Internal Node Splits (1 hr)
**Goal:** Handle cascading splits when internal nodes overflow.
**Algorithm:** Same idea — when an internal node is full and receives a pushed-up key, split the internal node and push a key further up. If the root splits, create a new root (tree grows taller).

**Implement:** Extend the insertion logic to handle internal splits recursively.
**Test:**
- Insert enough keys to force multiple levels of splits (e.g., 20+ keys with max 4 per node).
- Verify tree structure: all leaves at same level, all keys searchable.
- Print tree structure for visual verification.
**Done when:** The tree correctly handles cascading splits and maintains balance.

### Task 3.7 — B+ Tree: Range Scans (1 hr)
**Goal:** Implement sequential access through leaf nodes — crucial for range queries.
**System R (Section 3, "Images"):** "The leaf pages are chained in a doubly linked list, so that sequential access can be supported from leaf to leaf."

**Implement:**
```rust
pub fn range_scan(&self, start: &Value, end: &Value) -> Result<Vec<TupleId>>
```
**Algorithm:**
1. Search for the leaf containing `start`.
2. Scan forward through leaf entries, following next-leaf pointers.
3. Stop when key > `end` or no more leaves.

Also implement:
- `scan_all(&self) -> Result<Vec<(Value, TupleId)>>` — full index scan.
- `scan_from(&self, start: &Value) -> BTreeIterator` — returns an iterator.
**Test:**
- Insert 20 keys, range scan for keys 5..15, verify exactly the right keys come back.
- Full scan, verify all keys in sorted order.
**Done when:** Range scans return correct, ordered results across multiple leaf pages.

### Task 3.8 — B+ Tree: Deletion (1 hr)
**Goal:** Remove keys from the B+ tree.
**Simplified approach (acceptable for learning):** Mark entries as deleted (lazy deletion). Optionally implement merge/redistribute when a leaf falls below half-full.

**Implement:**
- `pub fn delete(&mut self, key: &Value) -> Result<bool>` — returns true if key was found and deleted.

**For the simple version:**
1. Find the leaf containing the key.
2. Remove the key-TID pair, shift remaining entries left.
3. Mark page dirty.
4. Do NOT merge or redistribute (simplification — System R itself notes this is a tuning issue).

**Test:**
- Insert 10 keys, delete 3, verify the remaining 7 are searchable.
- Range scan after deletion, verify deleted keys are absent.
**Done when:** Deletion works and does not corrupt the tree structure.

---

## Phase 4: SQL Parser (6 hours)

System R uses SEQUEL (later SQL). We'll implement a parser for a core subset.

### Task 4.1 — Rust: Strings, Iterators, and Peekable (1 hr)
**Read:** Rust Book Ch. 8.2 (Strings — UTF-8, slicing, chars).
**Read:** Rust Book Ch. 13 (Closures and Iterators) — focus on closures.
**Exercises:**
- Practice `str.chars().peekable()` — this is how you'll build the lexer.
- Write a function that takes a `&str` and splits it into words, handling quoted strings (e.g., `'hello world'` stays as one token).
- Understand `String` vs `&str` — when to own, when to borrow.
**Done when:** You can write a basic tokenizer that handles quoted strings.

### Task 4.2 — Lexer/Tokenizer (1 hr)
**Goal:** Create `src/sql/lexer.rs` — convert SQL text into tokens.
**Token types needed (from Appendix II of the paper):**
```rust
pub enum Token {
    // Keywords
    Select, From, Where, Insert, Into, Values,
    Update, Set, Delete, Create, Drop, Table,
    And, Or, Not, Order, By, Group, Having,
    Asc, Desc, Null, Unique, View, As, Join,
    // Literals
    Integer(i64), Float(f64), StringLit(String),
    // Identifiers
    Ident(String),
    // Operators
    Eq, Neq, Lt, Gt, Lte, Gte,
    Plus, Minus, Star, Slash,
    // Punctuation
    Comma, Semicolon, LParen, RParen, Dot,
    // Special
    Eof,
}
```
**Implement:** `pub fn tokenize(input: &str) -> Result<Vec<Token>>`
**Test:**
- `"SELECT name, sal FROM emp WHERE dno = 50"` produces correct token sequence.
- Handle string literals: `'PROGRAMMER'`.
- Handle numbers: `10000`, `3.14`.
**Done when:** Lexer correctly tokenizes all SQL examples from the paper.

### Task 4.3 — AST Definitions (1 hr)
**Goal:** Create `src/sql/ast.rs` — define the abstract syntax tree types.
**Based on Appendix II grammar, define:**
```rust
pub enum Statement {
    Select(SelectStmt),
    Insert(InsertStmt),
    Update(UpdateStmt),
    Delete(DeleteStmt),
    CreateTable(CreateTableStmt),
    DropTable(String),
}

pub struct SelectStmt {
    pub columns: Vec<SelectExpr>,  // or Star
    pub from: Vec<TableRef>,
    pub where_clause: Option<Expr>,
    pub group_by: Option<Vec<String>>,
    pub having: Option<Expr>,
    pub order_by: Option<Vec<OrderByExpr>>,
}

pub enum Expr {
    Column(String),                          // field name
    QualifiedColumn(String, String),         // table.field
    Literal(Value),
    BinaryOp { left: Box<Expr>, op: BinOp, right: Box<Expr> },
    UnaryOp { op: UnaryOp, expr: Box<Expr> },
    Function { name: String, args: Vec<Expr> },
    IsNull(Box<Expr>),
    // ... other variants as needed
}
```
**Done when:** AST types compile and can represent all the SQL examples from the paper (Examples 1-12).

### Task 4.4 — Parser: SELECT Statements (1 hr)
**Goal:** Create `src/sql/parser.rs` — parse SELECT statements.
**Implement a recursive descent parser:**
```rust
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn parse(input: &str) -> Result<Statement>
    fn parse_select(&mut self) -> Result<SelectStmt>
    fn parse_expr(&mut self) -> Result<Expr>
    fn parse_where(&mut self) -> Result<Option<Expr>>
}
```
**Parse these forms:**
- `SELECT * FROM emp`
- `SELECT name, sal FROM emp WHERE dno = 50`
- `SELECT * FROM emp WHERE job = 'CLERK' ORDER BY sal`
- `SELECT dno, COUNT(*) FROM emp GROUP BY dno HAVING COUNT(*) > 10`

**Test:** Parse each of the above and verify AST structure.
**Done when:** Parser handles SELECT with WHERE, ORDER BY, GROUP BY, HAVING.

### Task 4.5 — Parser: INSERT, UPDATE, DELETE (1 hr)
**Goal:** Parse data manipulation statements.
**Parse these forms (from paper Examples 5-8):**
- `INSERT INTO emp (empno, name, dno, job, sal) VALUES (100, 'Alice', 50, 'PROGRAMMER', 8500)`
- `UPDATE emp SET sal = sal * 1.1 WHERE dno = 50`
- `DELETE FROM emp WHERE dno = 50`

**Test:** Parse each and verify AST.
**Done when:** All DML statements parse correctly.

### Task 4.6 — Parser: CREATE TABLE, DROP TABLE (1 hr)
**Goal:** Parse DDL statements.
**Parse these forms (from paper Section 2, "Data Definition Facilities"):**
- `CREATE TABLE emp (empno INTEGER, name CHAR(50), dno INTEGER, job CHAR(20), sal INTEGER, mgr INTEGER)`
- `DROP TABLE emp`

**Supported types:** `INTEGER`, `SMALLINT`, `FLOAT`, `CHAR(n)`, `DECIMAL(p,s)`.
**Test:** Parse CREATE TABLE with various column types.
**Done when:** DDL parsing works for table creation and dropping.

---

## Phase 5: Query Execution Engine (8 hours)

System R executes queries using an iterator model — each operator produces tuples one at a time via OPEN/NEXT/CLOSE (the RSI cursor model from Section 2).

### Task 5.1 — Rust: Trait Objects and Dynamic Dispatch (1 hr)
**Read:** Rust Book Ch. 17.2 (Using Trait Objects That Allow for Values of Different Types).
**Read:** Rust Book Ch. 10.2 again — focus on trait bounds.
**Exercises:**
- Define a trait with a method, implement it on two different structs, store them in a `Vec<Box<dyn Trait>>`.
- Understand `Box<dyn Iterator<Item = T>>` — you'll return this from scan operators.
- Practice downcasting vs. just using trait methods.
**Done when:** You understand dynamic dispatch and can explain the performance tradeoff vs. generics.

### Task 5.2 — Executor Framework: The Volcano Iterator Model (1 hr)
**Goal:** Create `src/executor/mod.rs` — define the executor trait.
**System R (Section 2):** Tuples are materialized one at a time by the FETCH operator. Each call to FETCH delivers the next tuple.

**This maps to the Volcano iterator model:**
```rust
pub struct Tuple {
    pub values: Vec<Value>,
}

pub trait Executor {
    fn open(&mut self) -> Result<()>;
    fn next(&mut self) -> Result<Option<Tuple>>;
    fn close(&mut self) -> Result<()>;
    fn schema(&self) -> &Schema;
}
```

**Implement the first operator — `SeqScan`:**
```rust
pub struct SeqScan {
    table: Table,
    page_idx: usize,
    slot_idx: usize,
    // ... state for iterating through heap file pages
}
```
**Test:** Create a table with 10 rows, SeqScan it, verify all 10 returned.
**Done when:** SeqScan correctly iterates all tuples in a table via the Executor trait.

### Task 5.3 — Filter (Selection) Operator (1 hr)
**Goal:** Create `src/executor/filter.rs` — the WHERE clause executor.
**System R (Section 2):** "Tuples are selected by the WHERE clause."

**Implement:**
```rust
pub struct Filter {
    child: Box<dyn Executor>,
    predicate: Expr,
}
```
**Also implement expression evaluation:**
```rust
pub fn eval_expr(expr: &Expr, tuple: &Tuple, schema: &Schema) -> Result<Value>
```
- Handle comparisons: `=`, `<>`, `<`, `>`, `<=`, `>=`
- Handle boolean logic: `AND`, `OR`, `NOT`
- Handle `IS NULL`

**Test:**
- SeqScan → Filter(sal > 10000), verify only matching tuples returned.
- Filter with AND/OR predicates.
**Done when:** Filter correctly evaluates predicates and passes through only matching tuples.

### Task 5.4 — Projection Operator (1 hr)
**Goal:** Create `src/executor/project.rs` — the SELECT column list executor.
**Implement:**
```rust
pub struct Project {
    child: Box<dyn Executor>,
    expressions: Vec<Expr>,  // columns or expressions to output
    output_schema: Schema,
}
```
**Test:**
- SeqScan → Project(name, sal), verify output has only 2 columns.
- Project with expression: `sal * 1.1`.
**Done when:** Projection outputs only the requested columns/expressions.

### Task 5.5 — Index Scan Operator (1 hr)
**Goal:** Create `src/executor/index_scan.rs` — use B+ tree for lookups.
**System R (Section 3, "Images"):** "The RDS can rapidly fetch a tuple from an image by keying on the sort field values."

**Implement:**
```rust
pub struct IndexScan {
    btree: BTree,
    table: Table,
    start_key: Option<Value>,
    end_key: Option<Value>,
    // iterator state
}
```
- For equality: `start_key == end_key`
- For range: `start_key < end_key`
- For full scan: both None

**Test:**
- Create table with index on `dno`, insert 20 rows with various dno values.
- IndexScan for dno = 50, verify only matching rows returned.
- Range scan for dno between 10 and 30.
**Done when:** IndexScan correctly fetches tuples via the B+ tree.

### Task 5.6 — Nested Loop Join (1 hr)
**Goal:** Create `src/executor/join.rs` — join two relations.
**System R (Section 2, "Query Facilities"):** "The tables to be joined are listed in the FROM clause."

**Implement nested loop join (simplest, always applicable — Method 3 in paper):**
```rust
pub struct NestedLoopJoin {
    left: Box<dyn Executor>,
    right: Box<dyn Executor>,
    condition: Expr,
    output_schema: Schema,
    // state: current left tuple, right needs reset
}
```

**Key challenge:** The right side must be re-scanned for each left tuple. Either:
- Buffer all right tuples in memory, or
- Add a `reset()` method to the Executor trait.

**Test (Example 12 from paper):**
```sql
SELECT name, sal, dname FROM emp, dept
WHERE emp.job = 'PROGRAMMER' AND dept.loc = 'EVANSTON' AND emp.dno = dept.dno
```
**Done when:** Join produces correct results for a two-table query.

### Task 5.7 — Sort Operator (1 hr)
**Goal:** Create `src/executor/sort.rs` — ORDER BY support.
**Implement a simple in-memory sort:**
```rust
pub struct Sort {
    child: Box<dyn Executor>,
    order_by: Vec<(usize, SortDirection)>, // column index + ASC/DESC
    sorted_tuples: Vec<Tuple>,
    position: usize,
}
```
- `open()`: consume all child tuples, sort them.
- `next()`: return sorted tuples one at a time.

**Test:**
- Insert employees with various salaries, ORDER BY sal ASC, verify sorted output.
- ORDER BY sal DESC.
**Done when:** Sort correctly orders tuples by one or more columns.

### Task 5.8 — Aggregation Operator (GROUP BY, HAVING) (1 hr)
**Goal:** Create `src/executor/aggregate.rs`.
**System R (Section 2):** "Groups are formed by the GROUP BY clause. Groups are selected which satisfy the HAVING clause."

**Implement:**
```rust
pub struct Aggregate {
    child: Box<dyn Executor>,
    group_by: Vec<usize>,      // column indices to group by
    aggregates: Vec<AggFunc>,  // COUNT, SUM, AVG, MIN, MAX
    having: Option<Expr>,
}
```
**Aggregate functions:** COUNT, SUM, AVG, MIN, MAX, COUNT(*).
**Algorithm:** Consume all input, group by key columns, compute aggregates per group.

**Test (Example 2 from paper):**
```sql
SELECT dno FROM emp WHERE job = 'CLERK' GROUP BY dno HAVING COUNT(*) > 10
```
**Done when:** Aggregation correctly computes grouped results with HAVING filter.

---

## Phase 6: Query Planner (4 hours)

The planner converts a parsed SQL AST into an executor tree. The optimizer picks access paths.

### Task 6.1 — Simple Planner: AST to Executor Tree (1 hr)
**Goal:** Create `src/planner/mod.rs` — convert AST to a naive execution plan.
**For a SELECT statement, the plan is always:**
```
Project
  └─ Sort (if ORDER BY)
       └─ Aggregate (if GROUP BY)
            └─ Filter (if WHERE)
                 └─ Join or SeqScan (from FROM clause)
```

**Implement:**
```rust
pub fn plan(stmt: &Statement, db: &Database) -> Result<Box<dyn Executor>>
```
- Single table: SeqScan → Filter → Project
- Two tables: NestedLoopJoin → Filter → Project
- Always use SeqScan for now (no index selection yet).

**Test:** Plan and execute `SELECT name, sal FROM emp WHERE dno = 50`.
**Done when:** End-to-end works: SQL string → parse → plan → execute → results.

### Task 6.2 — DML Execution: INSERT, UPDATE, DELETE (1 hr)
**Goal:** Execute data manipulation through the planner.
**Implement executors for:**
- `InsertExecutor`: parse values, encode tuple, insert into heap file.
- `DeleteExecutor`: scan with filter, delete matching tuples.
- `UpdateExecutor`: scan with filter, update matching tuples.

**Test the full pipeline:**
```sql
CREATE TABLE emp (empno INTEGER, name CHAR(50), sal INTEGER);
INSERT INTO emp VALUES (1, 'Alice', 50000);
INSERT INTO emp VALUES (2, 'Bob', 60000);
SELECT * FROM emp;
UPDATE emp SET sal = 70000 WHERE name = 'Bob';
DELETE FROM emp WHERE empno = 1;
SELECT * FROM emp;
```
**Done when:** Full CRUD operations work through SQL.

### Task 6.3 — Cost-Based Access Path Selection (1 hr)
**Goal:** Implement the optimizer's access path selection from the paper (Section 2, "The Optimizer").
**System R considers these parameters:**
- R: relation cardinality (number of tuples)
- D: number of pages
- T: tuples per page (R/D)
- I: image cardinality (distinct values in index)
- H: CPU cost coefficient

**Implement the 8 methods from the paper for single-relation queries:**
```rust
pub fn choose_access_path(table: &TableInfo, predicates: &[Expr], indexes: &[IndexInfo]) -> AccessPath
```
- Method 1: Clustering image matching '=' predicate → cost R/(T*I)
- Method 7/8: Relation scan → cost R/T + H*R*N
- Choose the minimum cost method.

**Test:**
- Table with 1000 rows, index on `dno` with 20 distinct values.
- Query `WHERE dno = 50` should choose the index.
- Query `WHERE sal > 10000` (no matching index) should choose seq scan.
**Done when:** Optimizer picks index scan when an index matches and seq scan otherwise.

### Task 6.4 — Join Order Selection (1 hr)
**Goal:** For two-table joins, pick the better join order and method.
**System R (Section 2, "The Optimizer"):** Considers 4 methods for joins — we implement the two most important:
1. **Nested loop with index:** If there's an index on the join column of the inner table, use it.
2. **Sort-merge join:** Sort both sides on the join column, merge.

**Implement sort-merge join executor:**
```rust
pub struct SortMergeJoin {
    left: Box<dyn Executor>,
    right: Box<dyn Executor>,
    left_key: usize,
    right_key: usize,
}
```

**Optimizer:** If an index exists on the inner table's join column, use index nested loop. Otherwise, use sort-merge if both sides can be sorted on the join key. Fall back to nested loop join.

**Test (Example 12 from paper):**
- With index on emp.dno: optimizer should choose index nested loop.
- Without index: optimizer should choose sort-merge.
**Done when:** Optimizer picks appropriate join strategy based on available indexes.

---

## Phase 7: Transaction Management & Logging (6 hours)

System R provides full transaction support with BEGIN_TRANS, END_TRANS, SAVE, and RESTORE.

### Task 7.1 — Rust: Concurrency Primitives (1 hr)
**Read:** Rust Book Ch. 16 (Fearless Concurrency) — all sections.
**Exercises:**
- Practice `Mutex<T>` and `RwLock<T>` — understand when each is appropriate.
- Write a program with two threads incrementing a shared counter.
- Understand deadlock potential with multiple mutexes.
- Understand `Send` and `Sync` traits — why some types can't cross thread boundaries.
**Done when:** You can write a multi-threaded program that safely shares data.

### Task 7.2 — Write-Ahead Log (WAL) Structure (1 hr)
**Goal:** Create `src/log/mod.rs` — the transaction log.
**System R (Section 3, "Transaction Management"):** "The transaction recovery function is supported through the maintenance of time ordered lists of log entries, which record information about each change to recoverable data."

**Log record types:**
```rust
pub enum LogRecord {
    Begin { txn_id: u64 },
    Commit { txn_id: u64 },
    Abort { txn_id: u64 },
    Update {
        txn_id: u64,
        page_id: PageId,
        slot_id: u16,
        before_image: Vec<u8>,  // old value
        after_image: Vec<u8>,   // new value
    },
    Insert {
        txn_id: u64,
        page_id: PageId,
        slot_id: u16,
        data: Vec<u8>,
    },
    Delete {
        txn_id: u64,
        page_id: PageId,
        slot_id: u16,
        data: Vec<u8>,
    },
    Checkpoint { active_txns: Vec<u64> },
}
```

**Implement:**
- `LogManager::append(record: &LogRecord) -> Result<u64>` — returns LSN (Log Sequence Number).
- `LogManager::flush() -> Result<()>` — force log to disk.
- `LogManager::iter_from(lsn: u64) -> impl Iterator<Item = LogRecord>` — read log from a point.
- Write log to a separate file (e.g., `test.db.log`).
**Done when:** You can append log records and read them back.

### Task 7.3 — Transaction Manager (1 hr)
**Goal:** Create `src/transaction.rs` — manage transaction lifecycle.
**System R (Section 3):** "An RSS transaction is marked by the START_TRANS and END_TRANS operators."

**Implement:**
```rust
pub struct TransactionManager {
    next_txn_id: AtomicU64,
    active_txns: Mutex<HashMap<u64, TransactionState>>,
    log_manager: Arc<LogManager>,
}

pub enum TransactionState {
    Active,
    Committed,
    Aborted,
}

pub struct Transaction {
    pub txn_id: u64,
    // tracks pages modified by this txn
}
```
**Methods:**
- `begin() -> Result<Transaction>`
- `commit(txn: &Transaction) -> Result<()>` — write commit record, flush log.
- `abort(txn: &Transaction) -> Result<()>` — undo all changes, write abort record.

**WAL protocol:** Before a dirty page is written to disk, all log records for that page must be flushed first (the "write-ahead" rule).
**Done when:** Transactions can be started, committed, and aborted. Log records are written.

### Task 7.4 — Undo/Redo Recovery (1 hr)
**Goal:** Implement crash recovery by replaying the log.
**System R (Section 3, "System Checkpoint and Restart"):** On recovery:
1. **Redo:** Replay all committed transactions' changes.
2. **Undo:** Roll back all uncommitted transactions.

**Implement:**
```rust
pub fn recover(log: &LogManager, bpm: &mut BufferPoolManager) -> Result<()> {
    // 1. Scan log to find committed and active txns
    // 2. Redo all committed txns (apply after-images)
    // 3. Undo all uncommitted txns (apply before-images, in reverse)
}
```

**Test:**
1. Begin txn, insert 3 rows, commit. Begin txn, insert 2 rows, DO NOT commit.
2. Simulate crash (drop everything without flushing).
3. Recover: the 3 committed rows should be present, the 2 uncommitted should not.
**Done when:** Recovery correctly redoes committed and undoes uncommitted transactions.

### Task 7.5 — Integrate WAL with Buffer Pool (1 hr)
**Goal:** Modify `BufferPoolManager` to enforce the WAL protocol.
**Changes:**
1. Each page gets a `page_lsn` — the LSN of the last log record that modified it.
2. Before evicting a dirty page, flush the log up to `page_lsn`.
3. On page write-back, check that log is flushed.

**Also implement save points (System R's SAVE_TRANS):**
- `save(txn: &Transaction) -> Result<SavePointId>`
- `restore(txn: &Transaction, save_id: SavePointId) -> Result<()>` — undo to save point.

**Test:**
- Insert rows across multiple transactions.
- Force buffer eviction of dirty pages.
- Verify WAL protocol: log is always ahead of data pages on disk.
**Done when:** Buffer pool enforces WAL protocol. Save points work.

### Task 7.6 — Checkpoint (1 hr)
**Goal:** Implement system checkpoint for faster recovery.
**System R (Section 3, "System Checkpoint and Restart"):** "The Monitor then issues the SAVE_SEGMENT operator to bring disk copies of all relevant segments up to date."

**Implement:**
1. Write a `Checkpoint` log record listing all active transactions.
2. Flush all dirty pages to disk.
3. Flush the log.

**On recovery:** Start scanning from the last checkpoint instead of the beginning of the log.

**Test:**
- Run 100 transactions, checkpoint, run 10 more, simulate crash.
- Recovery should only need to process the 10 post-checkpoint transactions.
**Done when:** Checkpoints work and recovery uses them to limit log scanning.

---

## Phase 8: Concurrency Control (5 hours)

System R uses locking at multiple granularities — segments, relations, tuples, and pages.

### Task 8.1 — Lock Manager: Basic Structure (1 hr)
**Goal:** Create `src/concurrency/lock_manager.rs`.
**System R (Section 3, "Concurrency Control"):** "The RSS employs a single lock mechanism to synchronize access to all objects."

**Implement:**
```rust
pub enum LockMode {
    Shared,
    Exclusive,
    IntentShared,    // for hierarchical locking
    IntentExclusive, // for hierarchical locking
}

pub struct LockManager {
    lock_table: Mutex<HashMap<LockTarget, LockEntry>>,
}

pub enum LockTarget {
    Table(String),
    Page(PageId),
    Tuple(TupleId),
}

pub struct LockEntry {
    holders: Vec<(u64, LockMode)>,  // (txn_id, mode)
    waiters: VecDeque<(u64, LockMode, Sender<()>)>,
}
```
**Compatibility matrix:**
| | S | X | IS | IX |
|---|---|---|---|---|
| S | Y | N | Y | N |
| X | N | N | N | N |
| IS | Y | N | Y | Y |
| IX | N | N | Y | Y |

**Methods:**
- `lock(txn_id: u64, target: LockTarget, mode: LockMode) -> Result<()>` — may block.
- `unlock(txn_id: u64, target: LockTarget) -> Result<()>`
- `unlock_all(txn_id: u64) -> Result<()>` — release all locks for a transaction.
**Done when:** Lock manager correctly grants/blocks based on compatibility matrix.

### Task 8.2 — Two-Phase Locking (2PL) (1 hr)
**Goal:** Enforce 2PL protocol — transactions acquire all locks before releasing any.
**System R (Section 3):** Locks are held "until they are explicitly released or to the end of the transaction."

**Implement:**
- Track all locks held by each transaction.
- **Growing phase:** Transaction can acquire locks but not release.
- **Shrinking phase:** Once any lock is released, no more locks can be acquired.
- For simplicity, use **strict 2PL** — release all locks only at commit/abort.

**Integrate with TransactionManager:**
- `commit()` → release all locks.
- `abort()` → undo changes, then release all locks.

**Test:**
- Two transactions reading the same row: both get shared locks, both succeed.
- One transaction writes, another reads: writer gets exclusive lock, reader waits.
**Done when:** 2PL correctly serializes conflicting transactions.

### Task 8.3 — Deadlock Detection (1 hr)
**Goal:** Detect and resolve deadlocks.
**System R (Section 3):** "The detection is done by the Monitor, on a periodic basis, by looking for cycles in a user-user matrix."

**Implement a wait-for graph:**
```rust
pub struct DeadlockDetector {
    // wait-for graph: txn_id -> set of txn_ids it's waiting for
    wait_for: HashMap<u64, HashSet<u64>>,
}
```
- When a lock request blocks, add an edge to the wait-for graph.
- Periodically (or on each block), check for cycles using DFS.
- **Victim selection (from paper):** Choose the youngest transaction (highest txn_id).
- Abort the victim, releasing its locks.

**Test:**
- Set up a classic deadlock: T1 locks A, T2 locks B, T1 requests B, T2 requests A.
- Verify one transaction is aborted and the other proceeds.
**Done when:** Deadlock detection finds cycles and aborts the victim.

### Task 8.4 — Consistency Levels (1 hr)
**Goal:** Implement the three consistency levels from System R.
**System R (Section 3):**
- **Level 1:** No read locks. May read dirty data.
- **Level 2:** Short read locks (released after read). Clean reads but not repeatable.
- **Level 3:** Long read locks (held until end of txn). Full serializability.

**Implement:**
```rust
pub enum IsolationLevel {
    Level1, // ~ Read Uncommitted
    Level2, // ~ Read Committed
    Level3, // ~ Serializable
}
```
- Level 1: Skip shared locks on reads entirely.
- Level 2: Acquire shared lock, read, immediately release.
- Level 3: Acquire shared lock, hold until commit.

**Test:**
- Level 1: Transaction reads uncommitted data from another.
- Level 3: Transaction gets repeatable reads.
**Done when:** All three isolation levels work correctly.

### Task 8.5 — Integrate Locking with Executors (1 hr)
**Goal:** Wire locking into the query execution path.
**Changes:**
- SeqScan: acquire shared lock on table (intent shared on segment).
- IndexScan: acquire shared locks on accessed tuples.
- Insert/Update/Delete: acquire exclusive lock on affected tuples.
- All executors receive a `Transaction` reference.

**Modify Executor trait:**
```rust
pub trait Executor {
    fn open(&mut self, txn: &Transaction) -> Result<()>;
    fn next(&mut self, txn: &Transaction) -> Result<Option<Tuple>>;
    fn close(&mut self, txn: &Transaction) -> Result<()>;
}
```

**Test:**
- Run two concurrent transactions (using threads), verify correct isolation.
- One scanning, one inserting — verify no torn reads.
**Done when:** Executors correctly acquire locks and transactions are properly isolated.

---

## Phase 9: Interactive SQL Shell (3 hours)

The User-Friendly Interface (UFI) from System R — a standalone SEQUEL interface.

### Task 9.1 — Rust: I/O and CLI (1 hr)
**Read:** Rust Book Ch. 12 (Building a Command Line Program) — great practical chapter.
**Exercises:**
- Use `std::io::stdin()` to read lines in a loop.
- Parse command-line arguments with `std::env::args()`.
- Handle `Ctrl+C` gracefully.
**Done when:** You have a basic REPL that reads lines and echoes them.

### Task 9.2 — REPL: Read-Eval-Print Loop (1 hr)
**Goal:** Create `src/main.rs` — an interactive SQL shell.
**Implement:**
```
DucklingDB> CREATE TABLE emp (empno INTEGER, name CHAR(50), sal INTEGER);
Table 'emp' created.

DucklingDB> INSERT INTO emp VALUES (1, 'Alice', 50000);
1 row inserted.

DucklingDB> SELECT * FROM emp;
empno | name  | sal
------+-------+------
1     | Alice | 50000
(1 row)
```
**Features:**
- Multi-line input (wait for `;` to execute).
- Pretty-print results in a table format.
- Show row count after queries.
- Handle errors gracefully (show message, continue).
- `.quit` or `\q` to exit.

**Done when:** You can interactively create tables, insert data, and query it.

### Task 9.3 — DDL Commands and Index Management (1 hr)
**Goal:** Support CREATE/DROP INDEX through the shell.
**Implement SQL support for:**
```sql
CREATE INDEX idx_emp_dno ON emp (dno);
DROP INDEX idx_emp_dno;
```
- Wire CREATE INDEX to actually build a B+ tree over existing tuples.
- Register the index in the catalog.
- Future queries automatically consider the index in the optimizer.

**Also add:**
- `SHOW TABLES;` — list all tables.
- `DESCRIBE emp;` — show table schema.
- `EXPLAIN SELECT ...;` — show the query execution plan.

**Done when:** You can create indexes and see the optimizer use them via EXPLAIN.

---

## Phase 10: Testing, Hardening & Polish (5 hours)

### Task 10.1 — Comprehensive Storage Layer Tests (1 hr)
**Goal:** Achieve solid test coverage for the bottom layers.
**Write tests for:**
- DiskManager: concurrent reads/writes, large files, page boundaries.
- BufferPool: eviction under pressure, dirty page write-back, pin counting.
- SlottedPage: edge cases — page full, maximum size tuple, zero-length tuple.
- HeapFile: large inserts (hundreds of tuples spanning many pages), delete and reuse space.
**Done when:** All storage tests pass and cover edge cases.

### Task 10.2 — B+ Tree Stress Tests (1 hr)
**Goal:** Verify B+ tree correctness under stress.
**Tests:**
- Insert 10,000 keys in random order, verify all searchable.
- Delete half, verify remaining half searchable and deleted keys absent.
- Range scan entire tree, verify sorted order.
- Insert duplicate handling (should it error or store multiple TIDs?).
- Concurrent inserts from multiple threads (if locking is wired up).
**Done when:** B+ tree handles large datasets correctly.

### Task 10.3 — SQL End-to-End Tests (1 hr)
**Goal:** Write integration tests that run SQL strings through the full pipeline.
**Test cases (all from the paper's examples):**
```sql
-- Example 1b: Self-join
SELECT x.name, y.name FROM emp x, emp y WHERE x.mgr = y.empno AND x.sal > y.sal;

-- Example 2: Group by with having
SELECT dno FROM emp WHERE job = 'CLERK' GROUP BY dno HAVING COUNT(*) > 10;

-- Example 5: Set-oriented update
UPDATE emp SET sal = sal * 1.1 WHERE dno = 50;

-- Example 8: Subquery in delete (advanced — may skip)
DELETE FROM emp WHERE dno = (SELECT dno FROM dept WHERE loc = 'EVANSTON');
```
**Done when:** All paper examples that your parser supports execute correctly.

### Task 10.4 — Transaction and Recovery Tests (1 hr)
**Goal:** Verify ACID properties.
**Tests:**
- **Atomicity:** Begin, insert 5 rows, abort — verify no rows persisted.
- **Consistency:** Concurrent transfers between accounts preserve total balance.
- **Isolation:** Two concurrent readers see consistent snapshots (Level 3).
- **Durability:** Commit, simulate crash (kill process), restart, verify data present.
- **Recovery stress:** Run 50 random transactions (mix of committed/aborted), crash, recover, verify consistent state.
**Done when:** All ACID tests pass.

### Task 10.5 — Performance Benchmarking (1 hr)
**Goal:** Measure and document performance characteristics.
**Benchmarks:**
- Sequential insert throughput: tuples/second.
- Point lookup via index vs. seq scan.
- Range scan throughput.
- Join performance: nested loop vs. sort-merge.
- Transaction overhead: with and without logging.

**Use `std::time::Instant` for timing. Consider adding the `criterion` crate for proper benchmarks.**

**Document results in a comment or separate file. Identify bottlenecks.**
**Done when:** You have baseline performance numbers and understand the bottlenecks.

---

## Summary

| Phase | Topic | Hours |
|-------|-------|-------|
| 0 | Rust Foundations & Cleanup | 5 |
| 1 | Tuple Layout & Schema | 5 |
| 2 | System Catalog | 3 |
| 3 | B+ Tree Index | 8 |
| 4 | SQL Parser | 6 |
| 5 | Query Execution Engine | 8 |
| 6 | Query Planner & Optimizer | 4 |
| 7 | Transactions & Logging | 6 |
| 8 | Concurrency Control | 5 |
| 9 | Interactive SQL Shell | 3 |
| 10 | Testing & Polish | 5 |
| **Total** | | **58** |

### Recommended Order of Phases

Phases are designed to be done sequentially. Each builds on the previous one.

**Storage foundation (Phases 0-3):** Get comfortable with Rust while building the RSS (Relational Storage System) from the paper. You'll have a working storage engine with indexes.

**Query layer (Phases 4-6):** Build the RDS (Relational Data System) — parsing SQL, executing queries, and optimizing access paths. This is where DucklingDB becomes usable.

**ACID and concurrency (Phases 7-8):** Add transactions, recovery, and locking. These are harder but the paper gives excellent design guidance.

**User interface and polish (Phases 9-10):** Make it interactive and battle-tested.

### Tips
- Run `cargo test` after every task. Never move forward with failing tests.
- Run `cargo clippy` regularly — it catches common Rust mistakes.
- Commit after each task. Tag milestones (e.g., `git tag phase1-complete`).
- The Rust compiler is your friend. Read its error messages carefully — they're excellent.
- When stuck on Rust, check https://doc.rust-lang.org/std/ for standard library docs.
- Keep the System R paper open — almost every design decision maps to a section of the paper.
