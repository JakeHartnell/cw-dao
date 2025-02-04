START_DIR=$(pwd)

# ${f    <-- from variable f
#   ##   <-- greedy front trim
#   *    <-- matches anything
#   /    <-- until the last '/'
#  }
# <https://stackoverflow.com/a/3162500>

echo "generating schema for dao-dao-core"
cd contracts/dao-dao-core
cargo run --example schema > /dev/null
rm -rf ./schema/raw
cd "$START_DIR"

for f in ./contracts/delegation/*
do
  echo "generating schema for ${f##*/}"
  cd "$f"
  CMD="cargo run --example schema"
  eval $CMD > /dev/null
  rm -rf ./schema/raw
  cd "$START_DIR"
done

for f in ./contracts/distribution/*
do
  echo "generating schema for ${f##*/}"
  cd "$f"
  CMD="cargo run --example schema"
  eval $CMD > /dev/null
  rm -rf ./schema/raw
  cd "$START_DIR"
done

for f in ./contracts/voting/*
do
  echo "generating schema for ${f##*/}"
  cd "$f"
  CMD="cargo run --example schema"
  eval $CMD > /dev/null
  rm -rf ./schema/raw
  cd "$START_DIR"
done

for f in ./contracts/proposal/*
do
  echo "generating schema for ${f##*/}"
  cd "$f"
  CMD="cargo run --example schema"
  eval $CMD > /dev/null
  rm -rf ./schema/raw
  cd "$START_DIR"
done

for f in ./contracts/staking/*
do
  echo "generating schema for ${f##*/}"
  cd "$f"
  CMD="cargo run --example schema"
  eval $CMD > /dev/null
  rm -rf ./schema/raw
  cd "$START_DIR"
done

for f in ./contracts/pre-propose/*
do
  echo "generating schema for ${f##*/}"
  cd "$f"
  CMD="cargo run --example schema"
  eval $CMD > /dev/null
  rm -rf ./schema/raw
  cd "$START_DIR"
done

for f in ./contracts/external/*
do
  echo "generating schema for ${f##*/}"
  cd "$f"
  CMD="cargo run --example schema"
  eval $CMD > /dev/null
  rm -rf ./schema/raw
  cd "$START_DIR"
done
