wasm-pack build ../frontend --no-typescript --target web --out-dir ../build/pkg
cd ../backend
cargo build --release
cd ../build
mv ../backend/target/release/backend backend
./backend Config.toml
