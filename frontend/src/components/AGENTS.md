# Components AGENTS

## Kapsam
- `frontend/src/components/**`

## Kurallar
- Bileşenler öncelikle sunum ve kompozisyon katmanıdır.
- Raw fetch, doğrudan DB erişimi, config dosyası yazımı veya `systemctl` çağrısı burada bulunmamalı.
- Ortak UI için `components/ui/`, özellik bazlı akış için feature klasörlerini kullan.
- Veri alma/polling `hooks/` içinde, kalıcı yazma akışı API/lib katmanında kalmalı.
- Bir dosya 900 satıra yaklaşırsa yeni özelliği aynı dosyaya yığmak yerine alt bileşene ayır.
