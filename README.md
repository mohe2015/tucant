<!--
SPDX-FileCopyrightText: The tucant Contributors

SPDX-License-Identifier: AGPL-3.0-or-later
-->

<!-- Copyright (C) The tucant Contributors

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as published
by the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
GNU Affero General Public License for more details.

You should have received a copy of the GNU Affero General Public License
along with this program. If not, see <https://www.gnu.org/licenses/>. -->

<h1 align="center">
  TUCaN't

  [![GitHub license](https://img.shields.io/github/license/mohe2015/tucant.svg)](https://github.com/mohe2015/tucant/blob/main/LICENSE)
  [![GitHub commits](https://badgen.net/github/commits/mohe2015/tucant/main)](https://GitHub.com/mohe2015/tucant/commit/)
  [![Github stars](https://img.shields.io/github/stars/mohe2015/tucant.svg)](https://GitHub.com/mohe2015/tucant/stargazers/)
  [![CodeQL](https://img.shields.io/github/workflow/status/mohe2015/tucant/CodeQL?label=CodeQL)](https://github.com/mohe2015/tucant/actions/workflows/CodeQL.yml)
  [![Node.js CI](https://img.shields.io/github/workflow/status/mohe2015/tucant/Node.js%20CI?label=Node.js%20CI)](https://github.com/mohe2015/tucant/actions/workflows/node.js.yml)
  [![Rust](https://img.shields.io/github/workflow/status/mohe2015/tucant/Rust?label=Rust)](https://github.com/mohe2015/tucant/actions/workflows/rust.yml)
</h1>

A **nicer**, **faster** and more **featureful** frontend to <a href="https://www.tucan.tu-darmstadt.de/" target="_blank">TUCaN</a>.

## How it works

TUCaN't consists of three components: a fontend, a backend and a database. The frontend only communicates with the backend, which in turn communicates with the database. 

### Frontend

The frontend is written using [React](https://reactjs.org/) and [TypeScript](https://www.typescriptlang.org/). It should be a much faster, nicer looking and more featureful frontend to TUCaN.

### Backend

The backend is written in [Rust](https://www.rust-lang.org/) and is supposed to crawl TUCaN when first logging in. This data is then stored in a database to allow arbitrary analysis with it. There are also some web API endpoints for common things like navigating modules and full text search.

### Database

The database is a [PostgreSQL](https://www.postgresql.org/) database. It is used to store the crawled data from TUCaN.

## How to run

### Requirements

- [Docker](https://www.docker.com/)
- [Node.js](https://nodejs.org/en/)
- [NPM](https://www.npmjs.com/)
- [Rust](https://www.rust-lang.org/)
- [libpq-dev[_el_]](https://www.postgresql.org/docs/current/libpq.html) (might be called differently on other distributions)

### Database
```bash
cd backend-rust

# Depending on your system you might have to run these with sudo
docker build . -f Dockerfile-postgres --tag postgres-hunspell
docker run --name tucant-postgres -d --restart unless-stopped -e POSTGRES_INITDB_ARGS="--data-checksums" -e POSTGRES_PASSWORD=password -p 5432:5432 -it postgres-hunspell
```

### Backend

```bash
cd backend-rust

cargo install diesel_cli --no-default-features --features postgres
$HOME/.cargo/bin/diesel setup

# run this each time you want to run the backend
RUST_BACKTRACE=1 RUST_LOG=tucan_scraper=info,info cargo run
```

### Frontend

```bash
cd frontend-react

# install dependencies each time the package.json changed
npm ci

# run this each time you want to run the frontend
npm run dev
```

## Development Notes

If you want automatic formatting and linting on commit
```bash
ln -srf pre-commit.sh .git/hooks/pre-commit
```

If you want the backend to automatically restart on file change
```bash
cargo install cargo-watch
cargo watch -x check -s 'touch .trigger'
cargo watch --no-gitignore -w .trigger -x run
```

To test the backend
```bash
cd backend-rust
RUST_BACKTRACE=1 cargo test -- -Z unstable-options --nocapture --report-time
```

To get a nice GUI of the database on Linux
```bash
flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak install flathub io.dbeaver.DBeaverCommunity
```

To access the database from using a CLI on Linux  
`posgresql` needs to be installed on the host system 
```bash
psql postgres://postgres:password@localhost:5432/tucant
```

Add license headers  
`reuse` needs to be installed on the host system
```bash
reuse addheader --copyright "The tucant Contributors" --license AGPL-3.0-or-later --exclude-year --recursive --skip-unrecognised .
```
