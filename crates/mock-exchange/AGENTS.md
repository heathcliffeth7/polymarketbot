# mock-exchange AGENTS

## Kapsam
- `crates/mock-exchange/**`

## Kurallar
- Bu crate test double alanıdır; production logic taşımamalı.
- Davranış deterministik ve hafif kalmalı; gizli zaman/ağ bağımlılığı ekleme.
- Gerçek exchange semantiğini sadece repo testlerinin ihtiyaç duyduğu kadar taklit et.
- Fixture ve mock yanıtlarını açık tut; örtük global durumu büyütme.

## Doğrulama
- `cargo test -p mock-exchange`
