<h1 align="center">🛡️ Satsurance: ICP Canister</h1> <p align="center"><em>Secure, modular smart contract backend for the Internet Computer</em></p>
🚀 Local Setup Instructions
This guide walks you through setting up the Satsurance ICP Canister project locally, including dependencies, compilation, and running tests.

⚙️ Prerequisites
Ensure you have the following tools installed on your system:

🦀 Install Rust
bash
Copy
Edit
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
🧠 Install DFX (Internet Computer SDK)
bash
Copy
Edit
sh -ci "$(curl -fsSL https://internetcomputer.org/install.sh)"
🧩 Add the WebAssembly Target
bash
Copy
Edit
rustup target add wasm32-unknown-unknown
📥 1. Clone the Repository
bash
Copy
Edit
git clone https://github.com/Satsurance/canisters.git
cd canisters
🧪 2. Add Pocket-IC (Testing Emulator)
The project uses Pocket-IC v9 for local testing.

🔧 For macOS/Linux:
Download pocket_ic (v9) from the Pocket-IC Releases

Move the binary to:

bash
Copy
Edit
canisters/src/icp_canister_backend/
Rename the file to:

nginx
Copy
Edit
pocket_ic
Make the binary executable:

bash
Copy
Edit
chmod +x pocket_ic
🛠️ 3. Build the Project (WebAssembly)
Compile the Rust code into WebAssembly using:

bash
Copy
Edit
cargo build --target wasm32-unknown-unknown --release
🧪 4. Run Tests
Execute the unit and integration tests with:

bash
Copy
Edit
cargo test
✅ Summary
Rust + WebAssembly smart contract canister

Tested locally with Pocket-IC emulator

Easy CI-ready setup and modular project layout