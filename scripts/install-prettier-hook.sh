#!/bin/sh
# Installs a prettier gate into .git/hooks/pre-commit. Idempotent: re-running does not
# duplicate the hook (append-once by marker). The hook itself is not versioned, so this
# script is how the gate is restored after a fresh clone.
set -e

ROOT=$(git rev-parse --show-toplevel)
HOOK="$ROOT/.git/hooks/pre-commit"
MARKER="echo-prettier-gate"

if [ -f "$HOOK" ] && grep -q "$MARKER" "$HOOK"; then
  echo "prettier-гейт уже стоит в $HOOK — ничего не меняю."
  exit 0
fi

if [ ! -f "$HOOK" ]; then
  printf '#!/bin/sh\n' > "$HOOK"
  chmod +x "$HOOK"
fi

cat >> "$HOOK" <<'GATE'

# echo-prettier-gate: блокирует коммит, если staged-файлы не проходят prettier.
# Ставится scripts/install-prettier-hook.sh; повторная установка — append-once.
# ОГРАНИЧЕНИЕ, СКАЗАННОЕ ВСЛУХ: проверяется содержимое РАБОЧЕЙ КОПИИ, а не индекса.
# При частично застейдженном файле вердикт относится к файлу целиком. Полная
# проверка индекса требовала бы прогона каждого файла через stdin, а пайп в
# PowerShell/Git-Bash на Windows дописывает CRLF и ломает такой прогон молча —
# цена точности здесь выше выигрыша.
PRETTIER_STAGED=$(git diff --cached --name-only --diff-filter=ACM \
  -- '*.ts' '*.tsx' '*.js' '*.jsx' '*.mjs' '*.cjs' '*.json' '*.css' '*.md' '*.yml' '*.yaml' || true)
if [ -n "$PRETTIER_STAGED" ]; then
  ROOT=$(git rev-parse --show-toplevel)
  if [ -x "$ROOT/node_modules/.bin/prettier" ]; then
    # shellcheck disable=SC2086
    if ! (cd "$ROOT" && ./node_modules/.bin/prettier --check --ignore-unknown $PRETTIER_STAGED); then
      echo ""
      echo "✗ prettier: перечисленные выше файлы не отформатированы."
      echo "  Почини:  npm run format      (или прогони prettier --write по ним)"
      echo "  Иначе джоб 'code quality' покраснеет уже после пуша."
      exit 1
    fi
  else
    # Не тихий пропуск: гейт обязан сказать, что он НЕ отработал. Молчание здесь
    # неотличимо от «всё чисто» — ровно так format-гейт и протух в прошлый раз.
    echo "⚠ prettier не найден в node_modules — ГЕЙТ НЕ ОТРАБОТАЛ. Прогони 'npm install'."
  fi
fi
GATE

echo "prettier-гейт добавлен в $HOOK"
