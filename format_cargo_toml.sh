#!/bin/bash

rg --files -g Cargo.toml -g taplo.toml | sort -u | xargs taplo fmt $1
