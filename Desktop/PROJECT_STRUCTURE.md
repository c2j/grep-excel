# Project Structure

This document provides an overview of the template's structure and the purpose of each file and directory.

```
template/
├── README.md                    # Comprehensive documentation
├── QUICKSTART.md                # Quick start guide
├── PROJECT_STRUCTURE.md         # This file
├── index.html                   # HTML template
│
├── .gitignore                   # Git ignore rules
├── package.json                 # Node.js dependencies and scripts
├── vite.config.ts              # Vite configuration
├── tsconfig.json               # TypeScript configuration
├── tsconfig.node.json          # TypeScript configuration for Node
├── tailwind.config.js          # TailwindCSS configuration
├── postcss.config.js           # PostCSS configuration
│
├── public/                      # Static assets
│   ├── tauri.svg               # Tauri logo
│   └── vite.svg                # Vite logo
│
├── frontend/                    # Frontend source code (React + TypeScript)
│   ├── App.tsx                 # Main React application component
│   ├── index.tsx               # React entry point
│   ├── index.css               # Global styles with Tailwind directives
│   │
│   ├── api/                    # API layer for Tauri commands
│   │   └── tauri.ts           # Tauri API functions (greet, etc.)
│   ├── assets/                 # Static assets (images, fonts, etc.)
│   ├── components/             # Reusable React components
│   ├── hooks/                  # Custom React hooks
│   ├── store/                  # State management (Zustand, Redux, etc.)
│   ├── types/                  # TypeScript type definitions
│   │   ├── env.d.ts           # Vite environment types
│   │   └── index.ts           # Common type definitions
│   └── utils/                  # Utility functions
│
└── src-tauri/                  # Backend source code (Rust + Tauri)
    ├── .gitignore              # Rust project ignore rules
    ├── build.rs               # Tauri build configuration
    ├── Cargo.toml             # Rust dependencies and metadata
    ├── tauri.conf.json        # Tauri application configuration
    │
    ├── icons/                 # Application icons (for future use)
    │   └── (icon files)
    │
    └── src/                   # Rust source code
        ├── lib.rs             # Main library with Tauri commands
        └── main.rs            # Application entry point
```

## File Descriptions

### Root Level Files

- **README.md**: Comprehensive documentation including features, setup, and usage
- **QUICKSTART.md**: Quick start guide for rapid setup
- **PROJECT_STRUCTURE.md**: This file - explains the project structure
- **package.json**: NPM dependencies, scripts, and project metadata
- **index.html**: HTML entry point with root div and script reference
- **.gitignore**: Files and directories to ignore in version control

### Configuration Files

- **vite.config.ts**: Vite bundler configuration with Tauri-specific settings
- **tsconfig.json**: TypeScript configuration for the frontend
- **tsconfig.node.json**: TypeScript configuration for Node.js tools
- **tailwind.config.js**: TailwindCSS configuration with custom theme
- **postcss.config.js**: PostCSS configuration for processing CSS

### Frontend (frontend/)

The frontend directory contains all React/TypeScript code:

- **App.tsx**: Main application component demonstrating Tauri integration
- **index.tsx**: React entry point that mounts the app
- **index.css**: Global styles with Tailwind directives
- **types/index.ts**: Types and Tauri API imports (e.g., `greet` function)
- **types/env.d.ts**: Type definitions for Vite

### Backend (src-tauri/)

The backend directory contains Rust code and Tauri configuration:

- **Cargo.toml**: Rust dependencies (tauri, serde, etc.) and crate metadata
- **tauri.conf.json**: Application metadata, window settings, capabilities
- **build.rs**: Tauri build script configuration
- **src/main.rs**: Application entry point
- **src/lib.rs**: Library with Tauri commands and backend logic

## Key Concepts

### Tauri Commands

Commands are Rust functions that can be called from the frontend:

```rust
// src-tauri/src/lib.rs
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}
```

```typescript
// frontend/api/tauri.ts
import { invoke } from "@tauri-apps/api/tauri";

export async function greet(name: string): Promise<string> {
  return await invoke("greet", { name });
}
```

### Frontend Structure

The frontend follows a modular structure:

- **components/**: Reusable UI components
- **hooks/**: Custom React hooks for logic reuse
- **types/**: TypeScript type definitions
- **utils/**: Helper functions and utilities
- **store/**: State management (empty by default)
- **api/**: API layer for backend communication

### TailwindCSS Integration

TailwindCSS is configured and ready to use:

- Scanned files: `frontend/**/*.{js,ts,jsx,tsx}`
- Custom color palette: `primary` colors
- Direct utility classes in JSX

## Customization Guide

### Adding a New Tauri Command

1. Add command in `src-tauri/src/lib.rs`
2. Register it in `src-tauri/src/main.rs`
3. Create TypeScript wrapper in `frontend/types/index.ts`

### Adding a New Component

1. Create component in `frontend/components/`
2. Export from a barrel file (e.g., `frontend/components/index.ts`)
3. Import and use in `App.tsx` or other components

### Customizing Theme

Edit `tailwind.config.js`:

```js
module.exports = {
  theme: {
    extend: {
      colors: {
        primary: {
          // Your custom colors
        }
      }
    }
  }
}
```

### Modifying Window Settings

Edit `src-tauri/tauri.conf.json`:

```json
{
  "tauri": {
    "windows": [
      {
        "title": "Your App Name",
        "width": 1200,
        "height": 800,
        // ... other settings
      }
    ]
  }
}
```

## Development Workflow

1. **Edit Rust code** in `src-tauri/src/`
2. **Edit React code** in `frontend/`
3. **Run in development**: `npm run tauri:dev`
4. **Build for production**: `npm run tauri:build`

The template is designed to be minimal yet complete, providing a solid foundation for building desktop applications.
