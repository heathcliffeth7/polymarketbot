#!/usr/bin/env python3
"""
CLOB API credentials üreticisi.
Kullanım: CLAIMER_PRIVATE_KEY=0x... python scripts/gen_clob_creds.py [--update-config]
"""
import os, sys, re
from py_clob_client.client import ClobClient
from py_clob_client.constants import POLYGON

HOST = "https://clob.polymarket.com"
CONFIG_PATH = os.path.join(os.path.dirname(__file__), "../config/exchange.toml")

def main():
    private_key = os.environ.get("CLAIMER_PRIVATE_KEY")
    if not private_key:
        sys.exit("HATA: CLAIMER_PRIVATE_KEY env var ayarlanmamış")

    print("→ ClobClient L1 oturumu açılıyor...")
    client = ClobClient(host=HOST, key=private_key, chain_id=POLYGON)

    print("→ create_or_derive_api_key() çağrılıyor...")
    creds = client.create_or_derive_api_creds()

    api_key        = creds.api_key
    api_secret     = creds.api_secret
    api_passphrase = creds.api_passphrase

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

if __name__ == "__main__":
    main()
