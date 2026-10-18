#!/usr/bin/env bash
# Build the image, run the deployment with a certificate for localhost, prove
# the chain a browser uses, then tear everything down.
#
#     cd deploy && ./smoke.sh
#
# The run uses its own compose project, its own volumes and a private env file,
# so it never touches a deployment running beside it, and it leaves nothing
# behind. It builds the image, so it is the check CI runs.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

PROJECT="romeka-smoke-$$"
IMAGE="${ROMEEKA_SMOKE_IMAGE:-romeka-s:smoke}"
ADMIN_EMAIL="smoke-admin@example.org"
ADMIN_NAME="Smoke Admin"
ADMIN_PASSWORD="smoke-password"
HEALTH_TIMEOUT=300

# A free host port, so the stack can be brought up beside anything else on the
# machine and the base URL is known before the container starts.
free_port() {
    local port
    for _ in $(seq 1 50); do
        port=$(( (RANDOM % 40000) + 20000 ))
        if ! (exec 3<>"/dev/tcp/127.0.0.1/$port") 2>/dev/null; then
            printf '%s' "$port"
            return 0
        fi
    done
    echo "smoke: no free host port" >&2
    return 1
}

http_port="$(free_port)"
https_port="$(free_port)"
while [ "$http_port" = "$https_port" ]; do
    https_port="$(free_port)"
done
base="https://localhost:$https_port"

env_file="$(mktemp)"
compose() {
    docker compose --project-name "$PROJECT" --env-file "$env_file" "$@"
}

cleanup() {
    compose down --volumes --remove-orphans >/dev/null 2>&1 || true
    rm -f "$env_file"
}
trap cleanup EXIT

fail() {
    echo "smoke: $1" >&2
    compose logs --no-color romeka >&2 || true
    exit 1
}

# curl against the origin's own certificate, which no public CA signed.
get() {
    curl --silent --show-error --insecure --max-time 15 "$@"
}
code() {
    get --output /dev/null --write-out '%{http_code}' "$@"
}

cat > "$env_file" <<EOF
ROMEEKA_DOMAIN=localhost
ROMEEKA_BASE_URL=$base
ROMEEKA_ACME_EMAIL=smoke@localhost
ROMEEKA_HTTP_PORT=$http_port
ROMEEKA_HTTPS_PORT=$https_port
ROMEEKA_IMAGE=$IMAGE
ROMEEKA_ADMIN_EMAIL=$ADMIN_EMAIL
ROMEEKA_ADMIN_NAME=$ADMIN_NAME
ROMEEKA_ADMIN_PASSWORD=$ADMIN_PASSWORD
ROMEEKA_TITLE=Smoke Install
EOF

before="$(git -C .. status --porcelain)"

echo "== building $IMAGE and starting the stack on $base"
compose up --detach --build

echo "== waiting for $base/health"
deadline=$(( SECONDS + HEALTH_TIMEOUT ))
until [ "$(code "$base/health" 2>/dev/null || true)" = "200" ]; do
    if [ "$SECONDS" -ge "$deadline" ]; then
        fail "/health did not answer within ${HEALTH_TIMEOUT}s"
    fi
    sleep 2
done

health="$(get "$base/health")"
[ "$health" = '{"status":"ok"}' ] || fail "health body: $health"
echo "health:   $health"

ready="$(get "$base/ready")"
[ "$ready" = '{"status":"ready"}' ] || fail "ready body: $ready"
echo "ready:    $ready"

shell="$(get "$base/")"
case "$shell" in
    *'id="root"'*) ;;
    *) fail "the SPA shell is not at /: $(printf '%s' "$shell" | head -c 200)" ;;
esac
echo "spa:      the shell at / carries the app root"

bundle="$(printf '%s' "$shell" | sed -n 's/.*src="\([^"]*\.js\)".*/\1/p' | head -n 1)"
[ -n "$bundle" ] || fail "the shell names no script bundle"
bundle_code="$(code "$base/${bundle#./}")"
[ "$bundle_code" = "200" ] || fail "the bundle ${bundle#./} answered $bundle_code"
echo "bundle:   ${bundle#./}  $bundle_code"

# The store is a volume the server wrote; the origin serves it as files and
# never falls back to the SPA shell for it.
compose run --rm --no-deps --entrypoint /bin/sh romeka \
    -c 'mkdir -p "$ROMEEKA_STORE/original" && printf smoke > "$ROMEEKA_STORE/original/smoke.txt"'
stored="$(get "$base/files/original/smoke.txt")"
[ "$stored" = "smoke" ] || fail "the store answered: $stored"
missing="$(code "$base/files/original/missing.txt")"
[ "$missing" = "404" ] || fail "a missing store file answered $missing"
echo "store:    /files/original/smoke.txt  $stored, a miss is $missing"

headers="$(get --dump-header - --output /dev/null \
    --request POST --header 'content-type: application/json' \
    --data "{\"email\":\"$ADMIN_EMAIL\",\"password\":\"$ADMIN_PASSWORD\"}" \
    "$base/api/login")"
status="$(printf '%s' "$headers" | sed -n '1s/.* \([0-9][0-9][0-9]\) .*/\1/p')"
[ "$status" = "200" ] || fail "sign-in answered $status"
cookie="$(printf '%s' "$headers" | sed -n 's/^[Ss]et-[Cc]ookie: *\([^;]*\).*/\1/p' | head -n 1)"
[ -n "$cookie" ] || fail "sign-in set no cookie"
printf '%s' "$headers" | grep -qi '^set-cookie:.*secure' \
    || fail "the session cookie is not Secure over https"
identified="$(get --request POST --header 'content-type: application/json' \
    --data "{\"email\":\"$ADMIN_EMAIL\",\"password\":\"$ADMIN_PASSWORD\"}" \
    "$base/api/login")"
case "$identified" in
    *"$base/api/users/"*) ;;
    *) fail "the signed-in user's @id is not on the public origin: $(printf '%s' "$identified" | head -c 200)" ;;
esac
echo "sign-in:  $status, a Secure session cookie, @id on $base"

# The cookie write the browser makes: same origin passes, another origin does
# not. This is the behaviour a wrong base URL breaks.
same="$(code --request POST --header "Origin: $base" --header "Cookie: $cookie" "$base/api/logout")"
[ "$same" = "204" ] || fail "a same-origin logout answered $same"
other="$(code --request POST --header "Origin: https://evil.example" --header "Cookie: $cookie" "$base/api/logout")"
[ "$other" = "403" ] || fail "a cross-origin logout answered $other"
echo "origin:   a cookie write from $base is $same, from https://evil.example is $other"

after="$(git -C .. status --porcelain)"
[ "$before" = "$after" ] || fail "the repository changed during the run"

echo "== ok: the image builds, the origin terminates TLS, the server answers, the store is served, the administrator signs in, and a cookie write is refused from another origin"
