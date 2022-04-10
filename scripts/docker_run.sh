#!/bin/bash
docker run --mount type=bind,source=$(pwd),target=/host --mount type=bind,source=$(pwd)/tmp/docker/cargo_registry,target=/usr/local/cargo/registry --mount type=bind,source=$(pwd)/tmp/docker/target,target=/host/target -w /host --cap-add=SYS_PTRACE --security-opt seccomp=unconfined -i -t nearprotocol/contract-builder /bin/bash
