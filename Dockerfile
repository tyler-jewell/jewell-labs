# Portability proof: the whole system is `agent` + sh + curl + jq + llama.cpp.
# Build and run from a clean checkout (any small chat GGUF works):
#   docker build -t history-agents-test .
#   docker run --rm -v "$PWD":/repo:ro \
#     -v /path/to/model.gguf:/model.gguf:ro history-agents-test
FROM ghcr.io/ggml-org/llama.cpp:full
ENV LD_LIBRARY_PATH=/app
RUN apt-get update && \
    apt-get install -y --no-install-recommends jq curl procps && \
    rm -rf /var/lib/apt/lists/*
COPY portability-test.sh /portability-test.sh
ENTRYPOINT ["/bin/sh", "/portability-test.sh"]
