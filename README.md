<img width="966" height="646" alt="image" src="https://github.com/user-attachments/assets/684a0747-731b-4d76-b574-b901100c9d58" />



# FLTK-Screenshot
# FLTK Screenshot

> **Small. Fast. Practical.**
> **Screenshot. Annotate. Send.**

A professional screenshot tool built with **Rust + FLTK 1.5**.
No heavy Electron. No bloated KDE Spectacle. No 100 useless buttons.

Just: **Capture. Draw an arrow where the problem is. Send.**

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)
![FLTK](https://img.shields.io/badge/FLTK-1.5-green.svg)
![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20FreeBSD-lightgrey.svg)

---

## Why FLTK Screenshot?

There are hundreds of screenshot tools. They are all the same.
Heavy. Complicated. Slow.

**FLTK Screenshot is different.**

| Other Tools | FLTK Screenshot |
| :--- | :--- |
| Heavy - Electron / KDE | **Light - FLTK 1.5** |
| Slow startup - 2-3 seconds | **Instant startup** |
| 100 buttons you never use | **8 buttons you need** |
| English only | **Hebrew + English, Full RTL** |
| Bloated | **Small, Fast, Practical** |

KDE Spectacle is powerful, but it is heavy and complicated.
You open it to take one screenshot, and you get 100 options.

**FLTK Screenshot gives you exactly what you need. Nothing more.**

---

## Features

### 📸 Capture

- **Full Screen** - Capture the entire screen in one click
- **Select Area** - Drag with the mouse to capture any region
- **Window** - Capture a single window
- **Timer** - 3s / 5s delay for menus and tooltips

### ✏️ Annotate - Draw Where The Problem Is

- **Arrow** - Red arrow pointing to the bug. Instantly clear.
- **Rectangle** - Red box around the problem area
- **Blur / Pixelate** - Hide passwords, private data
- **Crop** - Cut to the important part
- **Undo** - `Ctrl+Z`

### 💾 Save & Share

- **Copy** - `Ctrl+C` - Copy to clipboard, paste anywhere
- **Save** - `Ctrl+S` - Save as PNG / JPG
- **Auto-naming** - `2026-11-21_14-30-00.png`

---

## Screenshots

### Main Window
- Clean toolbar on top
- Canvas in the middle
- Status bar at the bottom: `Ready • Drag to draw • Ctrl+Z Undo • Ctrl+C Copy • Ctrl+S Save`

### Annotation Example
- Red rectangle around the bug
- Red arrows pointing to the issue
- Blur over private information

---

## Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl+Z` | Undo |
| `Ctrl+C` | Copy to Clipboard |
| `Ctrl+S` | Save |
| `Esc` | Cancel Selection |

---

## Installation

### From Source - Cargo

```bash
git clone https://github.com/leumicard-code/fltk-screenshot.git
cd fltk-screenshot
cargo build --release
./target/release/fltk-screenshot
