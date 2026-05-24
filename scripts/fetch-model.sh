#!/usr/bin/env bash
#
# (Опционально) Загрузка ggml-моделей whisper из CLI в папку app data —
# туда же, куда их качает приложение через UI. В норме модели качаются прямо
# из интерфейса; этот скрипт удобен для предзагрузки в dev/CI.
#
#   ./fetch-model.sh            # large-v3-turbo (по умолчанию)
#   ./fetch-model.sh large-v3   # максимальное качество
#
# Идемпотентно: если файл уже есть — ничего не качает.
#
set -euo pipefail

MODEL="${1:-turbo}"
BASE_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main"
DEST_DIR="$HOME/Library/Application Support/com.vladislav.whisperdictate/models"

case "$MODEL" in
  turbo|large-v3-turbo)
    FILE="ggml-large-v3-turbo-q5_0.bin"
    ;;
  large-v3)
    FILE="ggml-large-v3.bin"
    ;;
  *)
    echo "Неизвестная модель: $MODEL (доступно: turbo | large-v3)" >&2
    exit 1
    ;;
esac

mkdir -p "$DEST_DIR"
DEST="$DEST_DIR/$FILE"

if [[ -f "$DEST" ]]; then
  echo "Уже на месте: $DEST"
  exit 0
fi

echo "Качаю $FILE → $DEST"
curl -L --fail --progress-bar "$BASE_URL/$FILE" -o "$DEST.part"
mv "$DEST.part" "$DEST"
echo "Готово: $DEST"
