# Tauri + React + TypeScript + TailwindCSS Template

A modern, batteries-included starter template for building desktop applications using:

- **[Tauri](https://tauri.app/)** - Build smaller, faster, and more secure desktop applications
- **[React](https://reactjs.org/)** - A JavaScript library for building user interfaces
- **[TypeScript](https://www.typescriptlang.org/)** - A strongly typed programming language that builds on JavaScript
- **[Vite](https://vitejs.dev/)** - Next generation frontend tooling
- **[TailwindCSS](https://tailwindcss.com/)** - A utility-first CSS framework

## Features

- ⚡️ Lightning-fast development with Vite
- 🔒 Type-safe development with TypeScript
- 🎨 Beautiful UI with TailwindCSS
- 🖥️ Native desktop app with Tauri
- 📦 Zero-config setup with sensible defaults
- 🚀 Production-ready build configuration

## Prerequisites

Before you begin, ensure you have the following installed:

- [Node.js](https://nodejs.org/) (version 16 or higher)
- [Rust](https://www.rust-lang.org/) (latest stable version)
- [Tauri CLI](https://tauri.app/v1/guides/getting-started/prerequisites)

### Install Tauri CLI

```bash
npm install --save-dev @tauri-apps/cli
# or
cargo install tauri-cli
```

## Getting Started

### 1. Clone or use this template

You can copy this template to your desired location:

```bash
cp -r /path/to/template /path/to/your/new/project
cd /path/to/your/new/project
```

Or clone it if it's in a git repository.

### 2. Install dependencies

```bash
npm install
```

### 3. Configure your application

Edit the following files to customize your application:

- `src-tauri/tauri.conf.json` - Application configuration (name, identifier, windows, etc.)
- `src-tauri/Cargo.toml` - Rust dependencies and metadata
- `package.json` - Node.js dependencies and scripts
- `tailwind.config.js` - TailwindCSS configuration

### 4. Run in development mode

```bash
npm run tauri:dev
```

This will start the Vite development server and launch the Tauri application.

### 5. Build for production

```bash
npm run tauri:build
```

This will create a production build of your application.

## Project Structure

```
.
├── src-tauri/           # Tauri backend (Rust)
│   ├── src/
│   │   ├── main.rs      # Application entry point
│   │   └── lib.rs       # Backend logic and Tauri commands
│   ├── Cargo.toml       # Rust dependencies
│   ├── build.rs         # Build configuration
│   └── tauri.conf.json  # Tauri configuration
│
├── frontend/            # Frontend (React + TypeScript)
│   ├── components/      # React components
│   ├── types/           # TypeScript type definitions
│   ├── utils/           # Utility functions
│   ├── App.tsx          # Main React component
│   ├── index.tsx        # React entry point
│   └── index.css        # Global styles
│
├── public/              # Static assets
├── index.html           # HTML template
├── package.json         # Node.js dependencies
├── vite.config.ts       # Vite configuration
├── tsconfig.json        # TypeScript configuration
├── tailwind.config.js   # TailwindCSS configuration
└── postcss.config.js    # PostCSS configuration
```

## Available Scripts

- `npm run dev` - Start development server (without Tauri)
- `npm run build` - Build frontend for production
- `npm run preview` - Preview production build
- `npm run tauri` - Run Tauri CLI commands
- `npm run tauri:dev` - Start development mode
- `npm run tauri:build` - Build for production

## Development

### Adding a new Tauri command

1. Add the command in `src-tauri/src/lib.rs`:

```rust
#[tauri::command]
fn my_command(value: String) -> String {
    format!("Received: {}", value)
}
```

2. Register the command in `src-tauri/src/main.rs`:

```rust
fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![my_command])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

3. Use the command in your React app:

Create a new file `frontend/api/tauri.ts`:

```typescript
import { invoke } from "@tauri-apps/api/tauri";

export async function myCommand(value: string): Promise<string> {
  return await invoke("my_command", { value });
}
```

Then import and use it in your component:

```typescript
import { myCommand } from "./api/tauri";

async function handleClick() {
  const result = await myCommand("Hello from React!");
  console.log(result);
}
```

### Styling with TailwindCSS

This template includes TailwindCSS with sensible defaults. You can:

1. Use utility classes directly in your JSX
2. Extend the theme in `tailwind.config.js`
3. Add custom styles in `frontend/index.css`

Example:

```jsx
<div className="bg-primary-500 text-white p-4 rounded-lg shadow-md">
  Hello, TailwindCSS!
</div>
```

### TypeScript Configuration

The template includes TypeScript with strict mode enabled. Type definitions for Tauri APIs are automatically available through the `@tauri-apps/api` package.

## Building for Production

### Desktop Applications

The build command will create platform-specific binaries:

```bash
npm run tauri:build
```

Build artifacts will be in `src-tauri/target/release/bundle/`.

### Customization

You can customize various aspects of your application:

- **App Name & Metadata**: Edit `src-tauri/tauri.conf.json`
- **Window Settings**: Modify the `windows` array in `tauri.conf.json`
- **Capabilities**: Configure `allowlist` in `tauri.conf.json`
- **Dependencies**: Update `Cargo.toml` for Rust and `package.json` for Node.js

## Learning Resources

- [Tauri Guides](https://tauri.app/v1/guides/)
- [React Documentation](https://reactjs.org/docs)
- [TypeScript Handbook](https://www.typescriptlang.org/docs)
- [Vite Guide](https://vitejs.dev/guide/)
- [TailwindCSS Documentation](https://tailwindcss.com/docs)

## Troubleshooting

### Rust version issues

Ensure you have the latest stable version of Rust:

```bash
rustup update stable
```

### Dependencies not installing

Try clearing the npm cache:

```bash
npm cache clean --force
rm -rf node_modules
npm install
```

### Build errors

For Rust build errors, try:

```bash
cd src-tauri
cargo clean
cargo update
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is open source and available under the [MIT License](LICENSE).

## Acknowledgments

- [Tauri Team](https://tauri.app/) for the amazing desktop app framework
- [Vite Team](https://vitejs.dev/) for the lightning-fast build tool
- [Tailwind Labs](https://tailwindcss.com/) for the utility-first CSS framework
