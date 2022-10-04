# Kickshort

### Setup Instructions
1. Install [rust](https://www.rust-lang.org/tools/install)
2. Clone the repo
`git clone https://github.com/KicksiteDev/kickshort`
3. Setup the environment variables
```
  export DATABASE_URL=postgresql://postgres:password@localhost:5432/kickshort
  export DATABASE_URL_TEST=postgresql://postgres:password@localhost:5432/kickshort_test
  export WHO_AM_I=localhost:8000
```
4. Install diesel
`cargo install disel@1.4.1 --no-features --features postgres`
5. Setup the DBs
`diesel setup`
`DATABASE_URL=$DATABASE_URL_TEST diesel setup`
6. `cargo run` to start the application
