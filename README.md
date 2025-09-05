# kame

A DIY text editor



https://github.com/user-attachments/assets/90f0f897-e337-44f5-acde-440ac30a3a8c



[Features](#features) - [Motivation](#motivation) - [Roadmap](#roadmap) - [Getting Started](#getting-started)

---

## Features

Here are some features that are present, planned, or in progress:

* Insert / delete text
* Saving files
* Basic movement commands (cursor movement, etc.)
* Incremental search
* Undo / Redo 
* Syntax highlighting (planned)

## Motivation

This project is meant to be a minimal, from-scratch text editor for learning and experimentation. The idea is to understand editor internals, build up from simple building blocks (buffer management, cursor movement, text insertion/deletion), and then layer more features as needed.

## Roadmap

Here are things that are either in progress or planned:

* Syntax highlighting
* Word wrap
* Directory/file browser integration
* User configuration

## Getting Started

### Requirements

* Rust toolchain (rustc, cargo)
* A modern OS (Unix / Linux / macOS; Windows may need adjustments)

### Installation

Clone the repository:

```bash
git clone https://github.com/Vuta/kame.git
cd kame
```

Build:

```bash
cargo build --release
```

### Usage

Run the editor:

```bash
./target/release/kame <path-to-file>
```

Basic commands (to be implemented / in progress):

* Move cursor: Ctrl-f/b/p/n - a/e
* Insert / delete text
* Save file: Ctrl-s
* Incremental search: Ctrl-r & enter to jump to result
* Undo / Redo: Ctrl-u / Ctrl-g

(As features are added, commands will evolve.)

---
