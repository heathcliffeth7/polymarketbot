export function formatClaimErrorForDisplay(error: string | null): string | null {
  const raw = String(error ?? '').trim();
  if (!raw) return null;

  const lower = raw.toLowerCase();
  if (
    lower.includes('activate funds') ||
    lower.includes('funds activation') ||
    lower.includes('relayer_wallet_activation_required')
  ) {
    return "Polymarket relayer funds activation istiyor. USDC.e bakiyeyi pUSD'ye activate et.";
  }
  if (looksLikeHtml(raw) || lower.includes('claim_relayer_adapter_invalid_html')) {
    return 'Relayer adapter beklenmeyen HTML hata sayfasi dondurdu. Servis URL/auth ayarlarini kontrol et.';
  }
  return raw;
}

function looksLikeHtml(value: string): boolean {
  const prefix = value.trimStart().slice(0, 256).toLowerCase();
  return (
    prefix.startsWith('<!doctype html') ||
    prefix.startsWith('<html') ||
    prefix.includes('<html')
  );
}
