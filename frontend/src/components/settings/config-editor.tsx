'use client';

import { useState, useEffect, useRef } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Switch } from '@/components/ui/switch';
import { useConfig, saveConfig } from '@/hooks/use-config';

const MASKED_SECRET = '********';

interface FieldDef {
  key: string;
  label: string;
  type: 'number' | 'text' | 'select' | 'boolean' | 'multiselect';
  options?: string[];
  min?: number;
  max?: number;
  step?: number;
}

interface ConfigEditorProps {
  file: string;
  title: string;
  fields: FieldDef[];
}

export function ConfigEditor({ file, title, fields }: ConfigEditorProps) {
  const { data } = useConfig(file);
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const saveTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isFirstLoad = useRef(true);

  useEffect(() => {
    if (data?.data) {
      isFirstLoad.current = true;
      setValues(data.data);
      setLoaded(true);
    }
  }, [data]);

  const isReadOnly = data && !data.writable;

  useEffect(() => {
    if (!loaded || isReadOnly) return;
    if (isFirstLoad.current) {
      isFirstLoad.current = false;
      return;
    }
    if (saveTimeoutRef.current) clearTimeout(saveTimeoutRef.current);
    saveTimeoutRef.current = setTimeout(async () => {
      setSaving(true);
      setError(null);
      try {
        await saveConfig(file, values);
        setSuccess(true);
        setTimeout(() => setSuccess(false), 2000);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Auto-save failed');
      } finally {
        setSaving(false);
      }
    }, 800);
    return () => {
      if (saveTimeoutRef.current) clearTimeout(saveTimeoutRef.current);
    };
  }, [values]); // eslint-disable-line react-hooks/exhaustive-deps

  const updateValue = (key: string, value: unknown) => {
    setValues((prev) => ({ ...prev, [key]: value }));
  };

  const updateSensitiveValue = (key: string, value: string) => {
    setValues((prev) => ({
      ...prev,
      [key]: value,
      [`has_${key}`]: value.trim() !== '',
    }));
  };

  const clearSensitiveValue = (key: string) => {
    setValues((prev) => ({
      ...prev,
      [key]: '',
      [`has_${key}`]: false,
    }));
  };

  return (
    <Card className="border-zinc-800 bg-zinc-900">
      <CardHeader className="flex flex-row items-center justify-between">
        <CardTitle className="text-sm font-medium text-zinc-400">{title}</CardTitle>
        <div className="flex items-center gap-2">
          {saving && <span className="text-xs text-zinc-400">Kaydediliyor...</span>}
          {!saving && success && <span className="text-xs text-emerald-400">Kaydedildi ✓</span>}
          {!saving && !success && error && <span className="text-xs text-red-400">{error}</span>}
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {fields.map((field) => (
          <div key={field.key} className="grid grid-cols-3 items-center gap-4">
            <Label className="text-zinc-300">{field.label}</Label>
            <div className="col-span-2">
              {field.type === 'boolean' ? (
                <Switch
                  checked={!!values[field.key]}
                  onCheckedChange={(v) => updateValue(field.key, v)}
                  disabled={isReadOnly}
                />
              ) : field.type === 'select' && field.options ? (
                <Select
                  value={String(values[field.key] ?? '')}
                  onValueChange={(v) => updateValue(field.key, v)}
                  disabled={isReadOnly}
                >
                  <SelectTrigger className="border-zinc-700 bg-zinc-800 text-zinc-200">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent className="border-zinc-700 bg-zinc-800">
                    {field.options.map((opt) => (
                      <SelectItem key={opt} value={opt} className="text-zinc-200">{opt}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              ) : field.type === 'multiselect' && field.options ? (
                <div className="flex flex-wrap gap-2">
                  {field.options.map((opt) => {
                    const selected = Array.isArray(values[field.key])
                      ? (values[field.key] as string[]).includes(opt)
                      : false;
                    return (
                      <label key={opt} className="flex items-center gap-1 cursor-pointer text-zinc-300 text-sm">
                        <input
                          type="checkbox"
                          checked={selected}
                          disabled={isReadOnly}
                          className="accent-emerald-500"
                          onChange={(e) => {
                            const cur = Array.isArray(values[field.key])
                              ? (values[field.key] as string[])
                              : [];
                            const next = e.target.checked
                              ? [...cur, opt]
                              : cur.filter((v) => v !== opt);
                            updateValue(field.key, next);
                          }}
                        />
                        {opt}
                      </label>
                    );
                  })}
                </div>
              ) : (
                <>
                  <Input
                    type={field.type === 'number' ? 'number' : 'text'}
                    value={String(values[field.key] ?? '')}
                    onChange={(e) => {
                      const v = field.type === 'number' ? parseFloat(e.target.value) || 0 : e.target.value;
                      if (isSensitiveExchangeField(file, field.key) && typeof v === 'string') {
                        updateSensitiveValue(field.key, v);
                        return;
                      }
                      updateValue(field.key, v);
                    }}
                    min={field.min}
                    max={field.max}
                    step={field.step}
                    disabled={isReadOnly}
                    className="border-zinc-700 bg-zinc-800 text-zinc-200"
                  />
                  {isSensitiveExchangeField(file, field.key) && (
                    <div className="mt-2 flex items-center justify-between text-xs">
                      <span className="text-zinc-500">
                        {String(values[field.key] ?? '') === MASKED_SECRET
                          ? 'Stored value is masked. Enter new value to replace.'
                          : 'Leave unchanged or clear explicitly.'}
                      </span>
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        disabled={isReadOnly}
                        className="h-7 px-2 text-zinc-400 hover:text-zinc-200"
                        onClick={() => clearSensitiveValue(field.key)}
                      >
                        Clear
                      </Button>
                    </div>
                  )}
                </>
              )}
            </div>
          </div>
        ))}
      </CardContent>
    </Card>
  );
}

function isSensitiveExchangeField(file: string, key: string): boolean {
  if (file !== 'exchange') return false;
  return ['api_address', 'api_key', 'api_secret', 'api_passphrase'].includes(key);
}
