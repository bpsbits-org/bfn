# BFN - PostgreSQL extension

Collection of instruments for particular PostgreSQL servers.

## Development

Composed in [Rust](https://www.rust-lang.org) and utilises the [pgrx](https://github.com/pgcentralfoundation/pgrx) framework.

Ensure that `Rust` and `pgrx` are installed on your systems, as they are essential for development. Instructions for utilising `Rust` and `pgrx` can be found on the aforementioned websites.

The build utilises a containerised version of `PostgreSQL`.

## Documentation

Please see [documentation](https://bpsbits.org/bfn/doc/bfn) for more details.

## Installation

The extension can be installed directly from the `GitHub` repository.

### Install extension files to server

Examples below require `curl`.

#### Inside server

Replace `pg-16-tes`t with name of your container.

```shell
curl -sL https://raw.githubusercontent.com/bpsbits-org/bfn/main/scripts/deploy-pg16.sh -o deploy-pg16.sh
bash ./deploy-pg16.sh
```

#### Install inside Podman container

Replace `pg-16-tes`t with name of your container.

```shell
podman exec --user root -it pg-16-test bash -c "curl -sL https://raw.githubusercontent.com/bpsbits-org/bfn/main/scripts/deploy-pg16.sh | bash"
```

### Install extension to database

Installs extension into schema `bfn`.

```sql
-- install bfn in schema bfn
create extension bfn schema bfn;
```

Check version:

```sql
-- Check bfn version
select bfn.bfn_version();
```

If the schema `bfn` does not exist, run:

```sql
-- Create schema bfn
create schema bfn authorization postgres;
```

### Upgrade extension in database

Please be advised that if you require an upgrade, you can create an upgrade file yourself.

In the provided example:

- `2.0.1` - version to upgrade 
- `2.0.0` - currently installed version

```shell
cp /usr/share/postgresql/16/extension/bfn--2.0.1.sql /usr/share/postgresql/16/extension/bfn--2.0.0--2.0.1.sql
```

