# Queries AGENTS

## Kapsam
- `frontend/src/lib/queries/**`

## Kurallar
- SQL builder ve read-model mantığı burada yaşar; kullanıcı girdisini string birleştirerek sorguya gömme.
- Dönen shape mevcut tiplerle uyumlu kalmalı; tip değişiyorsa aynı patchte güncelle.
- Feature klasör ayrımını koru; tek dosyayı büyütmek yerine alt modüle böl.
- `trade-flow/validation-actions.ts` zaten sınırda; yeni büyük mantığı bu dosyaya yığma.
- Şemaya bağlı query değişikliği migration ve backend kontratıyla birlikte gelmeli.
