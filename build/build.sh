wasm-pack build ../frontend --no-typescript --target web --out-dir ../build/pkg
cd ../backend
cargo build
cd ../build
mv ../backend/target/debug/backend backend
./backend Config.toml
