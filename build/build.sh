wasm-pack build ../frontend --no-typescript --target web --out-dir ../build/pkg
cd ../backend
cargo build --release --target x86_64-unknown-linux-musl 
cd ../build
mv ../backend/target/release/backend backend
./backend Config.toml
