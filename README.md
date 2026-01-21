<p align="center">
  <h1 align="center">Elio</h1>
  <p align="center">
    <strong>The Embedded Graph Database</strong>
  </p>
</p>

<p align="center">
  <a href="https://github.com/power1628/elio/actions/workflows/ci.yml">
    <img src="https://github.com/power1628/elio/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI">
  </a>
  <img src="https://img.shields.io/badge/license-Apache%202.0-blue.svg" alt="License">
  <img src="https://img.shields.io/badge/rust-nightly-orange.svg" alt="Rust">
</p>

> [!WARNING] 
> This project is under active development and is not yet stable. Do not use in production.

---

**Elio** is an embedded graph database written in Rust. It stores data locally
with RocksDB, requires no server setup, and lets you query using the familiar
**Cypher** query language.

```
elio> CREATE (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person {name: 'Bob'}) RETURN a, b
╭──────────────────────────────────────────────────────┬────────────────────────────────────────────────────╮
│ a                                                    │ b                                                  │
├──────────────────────────────────────────────────────┼────────────────────────────────────────────────────┤
│ {id: 2001, labels: [Person], props: {name: 'Alice'}} │ {id: 2002, labels: [Person], props: {name: 'Bob'}} │
╰──────────────────────────────────────────────────────┴────────────────────────────────────────────────────╯
1 row(s)
Executed in 0.009s
```

## Features

- **Embedded**: Link as a library or use the CLI. No Docker, no server process.
- **Cypher**: Use the query language you already know.
- **Fast**: Built with a native Rust execution engine and can process larger than memory workloads.

## Quick Start

### Installation

```bash
git clone https://github.com/elio-rs/elio.git
cd elio
make build
```

### Interactive CLI

```bash
cargo run -p cmd
```

```
Elio - An embedded graph database
Type .help for available commands, .quit to exit

elio> CREATE (n:Person {name: 'Alice', age: 30}) RETURN n
╭────────────────────────────────────────────────────────────╮
│ n                                                          │
├────────────────────────────────────────────────────────────┤
│ {id: 1, labels: [Person], props: {name: 'Alice', age: 30}} │
╰────────────────────────────────────────────────────────────╯
1 row(s)
Executed in 0.004s

elio> MATCH (n:Person) WHERE n.age > 25 RETURN n.name
╭─────────╮
│ n.name  │
├─────────┤
│ 'Alice' │
╰─────────╯
1 row(s)
Executed in 0.002s
```

### Use a Persistent Database

```bash
cargo run -p cmd -- --db-path ./my_graph.db
```

## Cypher Examples

```cypher
-- Create nodes
CREATE (a:Person {name: 'Alice', age: 30})

-- Create relationships
CREATE (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person {name: 'Bob'})

-- Query all nodes
MATCH (n) RETURN n

-- Filter by properties
MATCH (n:Person) WHERE n.age > 25 RETURN n

-- Pattern matching
MATCH (a:Person)-[:KNOWS]->(b:Person) RETURN a.name, b.name

-- Variable-length paths
MATCH (a)-[:KNOWS*1..3]->(b) RETURN a, b
```

## Contributing

Contributions are welcome! Feel free to open issues or submit pull requests.

## License

Apache 2.0
