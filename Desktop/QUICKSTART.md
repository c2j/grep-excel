# Quick Start Guide

Get your Tauri + React + TypeScript + TailwindCSS app up and running in 3 simple steps!

## Step 1: Setup

```bash
# 1. Copy the template to your desired location
cp -r /path/to/template /path/to/your/project
cd /path/to/your/project

# 2. Install dependencies
npm install

# 3. Install Rust dependencies (if not already installed)
cd src-tauri && cargo build && cd ..
```

## Step 2: Customize

Edit these files to personalize your app:

- `src-tauri/tauri.conf.json` - App name, description, identifier
- `src-tauri/Cargo.toml` - Rust crate name and version
- `package.json` - App name and description
- `tailwind.config.js` - Custom colors and theme

## Step 3: Run

```bash
# Development mode
npm run tauri:dev

# Production build
npm run tauri:build
```

## What You Get

✅ **Native Desktop App** - Built with Tauri for macOS, Windows, and Linux
✅ **Type-Safe Frontend** - React + TypeScript for robust UI development
✅ **Modern Tooling** - Vite for lightning-fast builds and hot reload
✅ **Beautiful Styling** - TailwindCSS for rapid UI development
✅ **Zero Config** - Sensible defaults out of the box

## Common Commands

```bash
# Start development server
npm run tauri:dev

# Build for production
npm run tauri:build

# Run Tauri CLI directly
npm run tauri -- [command]

# Build frontend only
npm run build

# Preview production build
npm run preview
```

## Next Steps

1. Read the full [README.md](./README.md) for detailed documentation
2. Add your first Tauri command in `src-tauri/src/lib.rs`
3. Start building your UI in `frontend/App.tsx`
4. Check out the [Tauri Guides](https://tauri.app/v1/guides/)

## Troubleshooting

**Rust not found?**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Tauri CLI not found?**
```bash
npm install --save-dev @tauri-apps/cli
# or
cargo install tauri-cli
```

**Port already in use?**
```bash
# Kill process on port 1420
lsof -ti:1420 | xargs kill
```

Happy coding! 🚀
