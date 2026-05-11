#!/usr/bin/env python3
"""
CLOB API credentials üreticisi.
Kullanım:
  CLAIMER_PRIVATE_KEY=0x... python scripts/gen_clob_creds.py [--host https://clob-v2.polymarket.com] [--update-config]
"""
import os
import re
import sys

try:
    from py_clob_client.client import ClobClient
    from py_clob_client.constants import POLYGON
except ImportError as exc:
    sys.exit(f"HATA: py-clob-client-v2 kurulu olmalı: {exc}")

DEFAULT_HOST = "https://clob-v2.polymarket.com"
CONFIG_PATH = os.path.join(os.path.dirname(__file__), "../config/exchange.toml")

def main():
    private_key = os.environ.get("CLAIMER_PRIVATE_KEY")
    if not private_key:
        sys.exit("HATA: CLAIMER_PRIVATE_KEY env var ayarlanmamış")

    host = _resolve_host(sys.argv[1:])
    print(f"→ ClobClient L1 oturumu açılıyor: {host}")
    client = _build_client(host, private_key)

    print("→ create_or_derive_api_key() çağrılıyor...")
    creds = _create_or_derive_api_key(client)

    api_key        = _get_cred(creds, "api_key")
    api_secret     = _get_cred(creds, "api_secret")
    api_passphrase = _get_cred(creds, "api_passphrase")

    print("\n=== YENİ CREDENTIALS ===")
    print(f"api_key        = {api_key}")
    print(f"api_secret     = {api_secret}")
    print(f"api_passphrase = {api_passphrase}")

    if "--update-config" in sys.argv:
        _update_toml(api_key, api_secret, api_passphrase)
        print(f"\n✓ {CONFIG_PATH} güncellendi")

def _update_toml(key, secret, passphrase):
    with open(CONFIG_PATH, "r") as f:
        content = f.read()
    content = re.sub(r'api_key\s*=\s*"[^"]*"',        f'api_key = "{key}"',        content)
    content = re.sub(r'api_secret\s*=\s*"[^"]*"',     f'api_secret = "{secret}"',  content)
    content = re.sub(r'api_passphrase\s*=\s*"[^"]*"', f'api_passphrase = "{passphrase}"', content)
    with open(CONFIG_PATH, "w") as f:
        f.write(content)

def _resolve_host(args):
    if "--host" in args:
        idx = args.index("--host")
        try:
            return args[idx + 1]
        except IndexError:
            sys.exit("HATA: --host değeri eksik")
    return os.environ.get("CLOB_HOST", DEFAULT_HOST)

def _build_client(host, private_key):
    try:
        return ClobClient(host=host, key=private_key, chain=POLYGON)
    except TypeError:
        return ClobClient(host=host, key=private_key, chain_id=POLYGON)

def _create_or_derive_api_key(client):
    for method_name in ("create_or_derive_api_key", "create_or_derive_api_creds"):
        method = getattr(client, method_name, None)
        if method:
            return method()
    sys.exit("HATA: py-clob-client-v2 create_or_derive_api_key methodunu sağlamıyor")

def _get_cred(creds, key):
    if isinstance(creds, dict):
        return creds.get(key, "")
    return getattr(creds, key, "")

if __name__ == "__main__":
    main()
