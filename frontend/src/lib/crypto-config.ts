import { createCipheriv, createDecipheriv, randomBytes } from 'crypto';

const ENC_PREFIX = 'enc:v1:';
const NONCE_LENGTH = 12;
const TAG_LENGTH = 16;

function getConfigEncryptionKey(): Buffer {
  const encoded = process.env.CONFIG_ENCRYPTION_KEY?.trim();
  if (!encoded) {
    throw new Error(
      'CONFIG_ENCRYPTION_KEY is required for credential encryption (base64 encoded 32-byte key).'
    );
  }

  let decoded: Buffer;
  try {
    decoded = Buffer.from(encoded, 'base64');
  } catch {
    throw new Error('CONFIG_ENCRYPTION_KEY must be valid base64.');
  }

  if (decoded.length !== 32) {
    throw new Error('CONFIG_ENCRYPTION_KEY must decode to exactly 32 bytes.');
  }

  return decoded;
}

export function isEncryptedConfigValue(value: string): boolean {
  return value.startsWith(ENC_PREFIX);
}

export function encryptConfigValue(plainText: string): string {
  const key = getConfigEncryptionKey();
  const nonce = randomBytes(NONCE_LENGTH);
  const cipher = createCipheriv('aes-256-gcm', key, nonce);

  const encrypted = Buffer.concat([
    cipher.update(Buffer.from(plainText, 'utf8')),
    cipher.final(),
  ]);
  const tag = cipher.getAuthTag();
  const payload = Buffer.concat([nonce, encrypted, tag]).toString('base64');

  return `${ENC_PREFIX}${payload}`;
}

export function decryptConfigValue(encryptedValue: string): string {
  if (!isEncryptedConfigValue(encryptedValue)) {
    return encryptedValue;
  }

  const key = getConfigEncryptionKey();
  const payload = encryptedValue.slice(ENC_PREFIX.length);
  const decoded = Buffer.from(payload, 'base64');
  if (decoded.length <= NONCE_LENGTH + TAG_LENGTH) {
    throw new Error('Encrypted config payload is malformed.');
  }

  const nonce = decoded.subarray(0, NONCE_LENGTH);
  const cipherText = decoded.subarray(NONCE_LENGTH, decoded.length - TAG_LENGTH);
  const tag = decoded.subarray(decoded.length - TAG_LENGTH);

  const decipher = createDecipheriv('aes-256-gcm', key, nonce);
  decipher.setAuthTag(tag);

  const plain = Buffer.concat([decipher.update(cipherText), decipher.final()]);
  return plain.toString('utf8');
}

export function getEncryptedPrefix(): string {
  return ENC_PREFIX;
}
