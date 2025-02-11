-- Check server version
select version();

-- Create db test
create database test;

-- Create schema bfn
create schema bfn authorization postgres;

-- Check is bfn installed
select *
from pg_extension
where
    extname = 'bfn';

-- Check is bfn available for install
select *
from pg_available_extensions
where
    name = 'bfn';

-- install bfn in schema bfn
create extension bfn schema bfn;

-- Check bfn version
select bfn.bfn_version();

-- Generate new uuid
select bfn.new_uuid();
