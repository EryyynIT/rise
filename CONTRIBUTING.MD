# 💻 Contributing to rise

> No corporate bureaucracy, just Rust and vibes.

We're building this thing together. Found a panic? Have a killer template idea? Maybe you just want to build this thing too? Awesome. Here's how to help without losing your mind.

---

## 🐛 Found a Bug?

**Please open an issue and tell us:**
- What you were doing when everything went sideways
- What you expected to happen
- What actually happened (panic messages are gold)
- Your Rust version (`rustc --version`)

*Extra points for:*
- A minimal reproduction
- Stack traces
- Screenshots of the carnage

## 💡 Got a Feature Idea?

**We love ideas! Open an issue and:**
- Explain the problem you're trying to solve
- Suggest how rise could help
- Bonus: propose how it might work

*Template ideas are especially welcome. Got a hot take on the perfect web app starter? We want to hear it.*

---

## 🛠️ Hands Dirty Time

### Setup for Development

```bash
# Clone the thing
git clone https://github.com/EryyynIT/rise
cd rise

# Build it (hope it works)
cargo build

# Run it (hope it works better)
cargo run

# Test it (please let the tests pass)
cargo test
```

### Making Changes

1. **Fork it** → Make a branch → Do your thing
2. **Keep it simple** - small PRs are easier to review
3. **Test your code** - or at least run it once before submitting
4. **Follow the existing code style** - we're not picky, just consistent

### Project Structure (The Important Bits)

```
src/
├── main.rs          # Where the magic starts
├── lib.rs           # Where the real magic lives
├── templates/       # Project templates go here
├── cli/             # Command line stuff
├── color/           # Main palette
└── config/          # Configuration nonsense
```

### Adding New Project Types

Want to add support for that obscure Rust use case we haven't thought of? 

1. Add your variant to `ProjectConfig` in `src/config/project.rs`
2. Implement the template generation in `src/templates/`
3. Update the CLI prompts in `src/cli/`
4. Submit a PR and bask in the glory

---

## 🎯 Code Philosophy

- **It should work** - ideally without panicking
- **Keep it fast** - nobody likes waiting
- **Clear over clever** - we're all tired
- **Async when it makes sense** - but don't force it
- **Batteries included, but swappable** - sensible defaults, escape hatches available

---

## 💬 Communication

- **Be excellent to each other** - it's that simple
- **Assume good intent** - we're all here to make cool Rust stuff
- **No gatekeeping** - Rust is for everyone

---

## 📝 License

By contributing, you agree that your contributions will be licensed under the project's Apache 2.0 License. 

*Translation: we're all sharing this thing, so play nice.*

---

**Questions?** Open an issue or just dive in. We don't bite (unless you break the build).

Remember: It's not official. It's better.
