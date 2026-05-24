# Contributing to egakareta

First off, thank you for considering contributing to egakareta! It's people like you that make egakareta a great game.

## Code of Conduct

We are committed to providing a friendly, safe, and welcoming environment for all. Please be respectful and considerate of others in all interactions.

## How Can I Contribute?

- **Reporting Bugs:** Open an issue with a clear description and steps to reproduce.
- **Suggesting Enhancements:** Open an issue describing the feature and its value.
- **Code Contributions:** Fix bugs or implement features from the issue tracker.
- **Localization:** We plan to support localization in the future.
- **Questions?** Contact us at [team@egakareta.com](mailto:team@egakareta.com).

## Development Environment Setup

### Via Github Codespaces

[![Open in GitHub Codespaces](https://github.com/codespaces/badge.svg)](https://codespaces.new/egakareta/egakareta)

### Via Local Setup

#### Prerequisites

- [Bun](https://bun.sh)
- [Rustup](https://rustup.rs)

#### Initial Setup

```bash
# Clone the repository
git clone https://github.com/egakareta/egakareta.git
cd egakareta

# Install dependencies
bun install

# Set up the local development environment
bun run dev:setup

# Run the development server at http://localhost:8788
bun run dev
```

## Pull Request Process

1. Fork the repo and create your branch from `main`.
2. Ensure the code builds and tests pass.
3. Run `bun run lint` and `bun run format`.
4. Submit a PR with a clear description of the changes.
5. Once your PR is merged, we will update the version as needed.

## License

By contributing to egakareta, you agree that your contributions will be licensed under its dual GNU AGPLv3 and Commercial License.
