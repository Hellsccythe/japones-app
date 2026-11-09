#!/bin/bash
# Avaliação da Fase 0: baixa o que faltar e roda os 4 modelos em sequência.
cd /e/dev/models
curl -sL -C - --retry 5 -o gemma-4-E2B_q4_0-it.gguf https://huggingface.co/google/gemma-4-E2B-it-qat-q4_0-gguf/resolve/main/gemma-4-E2B_q4_0-it.gguf
cd "$(dirname "$0")"
mkdir -p results; rm -f results/DONE
for m in Qwen3.5-2B-Q4_K_M Qwen3.5-4B-Q4_K_M gemma-4-E2B_q4_0-it gemma-4-E4B_q4_0-it; do
  ./target/release/spike-llm.exe /e/dev/models/$m.gguf words.txt results/$m.jsonl 3 8 > results/$m.log 2>&1
done
touch results/DONE
