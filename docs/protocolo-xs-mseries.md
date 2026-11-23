# Protocolo de Comunicación Avery Berkel — Serie XS (`MSeriesProtocol`)

> **Estado:** Validado en vivo (lectura, alta, edición, cambio rápido de precio y eliminación) contra báscula física **XS Series** (`192.168.9.113:3001`).  
> **Dispositivo probado:** `M Series Scale` | `APP. v4.1.7.0` | `Xs Series ABR30-PROTOCOL 8000` (`xsp8000.dri`).

---

## 1. Capa de Transporte

| Parámetro | Valor |
|---|---|
| Transporte | **TCP/IP** (`TCP_NODELAY = true`, `KeepAlive = true`) |
| Puerto de datos | **`3001`** (`MSeriesDataPort`) |
| Puerto de descubrimiento LAN | **UDP `23452`** (`:AVERYBERKELENQ:`) |
| Puerto de servicio (diagnóstico/firmware) | `28822` (`XsServicePort`) |
| Endianness (cabecera y payloads) | **Big-Endian** (Network Byte Order) |
| Codificación de texto | `Windows-1252` (CP1252) |
| Tamaño máximo de bloque de datos | **500 bytes** por bloque (`MAX_DATA_BLOCK_SIZE`) |

> **Reglas críticas de TCP:**
> 1. Cada bloque (`ABcmdheader` de 51 bytes + `Payload`) **debe enviarse en una sola llamada `Write()` / `send()`**. Dividir cabecera y payload en dos escrituras separadas con `TCP_NODELAY = true` provoca timeout en la báscula XS.
> 2. **Reutilización de conexión (sesión persistente):** El stack TCP embebido de la serie XS puede rechazar o bloquear temporalmente el puerto `3001` si un cliente abre y cierra muchas conexiones TCP en ráfaga. Se recomienda mantener **una sola conexión TCP abierta por sesión** (igual que `MSeriesDataPort` en MXBusiness) y cerrarla solo al terminar o cambiar de báscula.

---

## 2. Estructura de Trama (`ABcmdheader` — 51 bytes fijos)

| Offset | Bytes | Campo | Tipo | Descripción / Valores |
|---:|---:|---|---|---|
| `0..29` | 30 | `ContBlock` | `byte[30]` | 30 bytes en `0x00` |
| `30..31` | 2 | `commandVersionNumber` (CVN) | `uint16` BE | `0` (`VERSION_DETAILS`), `11` (`DATE_AND_TIME`), `3400` (`CHANGE_PRICE`), `5000` (`BARCODE_FORMATS`), `5200` (`LABEL_FORMATS`), `7600` (`PLU_RECORD`), `8000` (`ASSETINFO`, `MACHNETCONFIG`) |
| `32..33` | 2 | `length` | `uint16` BE | **`19 + payload.Length`** |
| `34` | 1 | `source_id` | `uint8` | `99` (`0x63` = Host) |
| `35` | 1 | `dest_id` | `uint8` | `253` (`0xFD` = Self), `254` = Server, `255` = Broadcast |
| `36` | 1 | `family` | `uint8` | `32` (`0x20` = `M_SERIES`) |
| `37` | 1 | `command_or_response` | `uint8` | `'I'` (`73`) = Petición; `'R'` (`82`) = Respuesta |
| `38` | 1 | `channel` | `uint8` | `0` = `Main` |
| `39..40` | 2 | `command_type` | `uint16` BE | `5`=`CHANGE_PRICE`, `6`=`STORED_TEXT`, `7`=`BARCODE_FORMATS`, `13`=`PLU_RECORD`, `16`=`DATE_AND_TIME`, `19`=`STORED_TARE`, `20`=`TAX_RATES`, `24`=`MACHINE_PARAMETERS`, `29`=`LABEL_FORMATS`, `37`=`MEDIA/LOGOS`, `39`=`DEPT_NAMES`, `59`=`VERSION_DETAILS`, `86`=`NETWORK_PARAMETERS`, `121`=`ASSETINFO`, `128`=`MACHNETCONFIG` |
| `41` | 1 | `action` | `uint8` | `'Q'` (`81`) = `REQUEST`, `'U'` (`85`) = `UPDATE`, `'C'` (`67`) = `CLEAR` |
| `42..43` | 2 | `status` | `int16` BE | `0` = `SUCCESS`, códigos negativos = error |
| `44` | 1 | `blockNum` | `uint8` | Bloque actual (1-based) |
| `45` | 1 | `numBlocks` | `uint8` | Total de bloques del mensaje |
| `46` | 1 | `endian` | `uint8` | `'N'` (`78`) = `NETWORK` (Big-Endian) |
| `47..50` | 4 | `sendsequence` | `uint32` BE | Contador de secuencia incremental |

---

## 3. Clave Compuesta `PluKey` (8 bytes BE)

```text
PluKey = ((uint64)DID << 56) | PLU
```

- **Byte 0:** `DID` (`0..99`).
- **Bytes 1..7:** `PLU` (entero sin signo de 56 bits BE, `1..9999999999999`).

---

## 4. Mapa Completo del Registro `PLU_RECORD` (`CMD = 13`, `CVN = 7600`)

El payload de `PLU:READ` (respuesta) y `PLU:WRITE` (petición) se compone de:
1. **Bytes `0x00..0x28` (41 bytes):** Cabecera de clave/búsqueda (`FromKey` 8B, `ToKey` 8B, `EAN` 8B en `WRITE` o ceros en `READ`, `EAN` 8B en `WRITE` o ceros en `READ`, `SearchIndex` 4B en `READ`, 3B ceros, `00 63` en `READ`/`DELETE` o `00 00` en `WRITE`).
2. **Bytes `0x29..0x7B` (83 bytes):** Registro binario fijo (`rec = payload + 41`):

| Offset `rec` | Offset `payload` | Bytes | Tipo | Campo (`xsp8000.dri`) | Descripción / Codificación |
|---:|---:|---:|---|---|---|
| `0x00..0x07` | `0x29..0x30` | 8 | `PluKey` BE | `DID` + `PLU` | Departamento (byte 0) + Número PLU (bytes 1..7) |
| `0x08..0x09` | `0x31..0x32` | 2 | `uint16` BE | `PTV` | Proportional Tare Value (`0..9999`) |
| `0x0A..0x0B` | `0x33..0x34` | 2 | `uint16` BE | `NWV` / `NV` | Peso nominal (`0..9999`) o volumen nominal |
| `0x0C` | `0x35` | 1 | `uint8` | `LGID` | Group ID (`0..99`) |
| `0x0D..0x10` | `0x36..0x39` | 4 | `uint32` BE | **`P1`** | Precio 1 en centavos (`7000` = \$70.00) |
| `0x11` | `0x3A` | 1 | `uint8` | `PMN` | Price Modifier Multiplier (default `1`) |
| `0x12..0x15` | `0x3B..0x3E` | 4 | `uint32` BE | **`P2FS`** | Precio 2 en centavos |
| `0x16..0x19` | `0x3F..0x42` | 4 | `uint32` BE | **`P3`** | Precio 3 en centavos |
| `0x1A` | `0x43` | 1 | `uint8` | **`CID`** | Container / Tare ID (`0..39`) |
| `0x1B` | `0x44` | 1 | `uint8` | **`PFID`** | Print Format ID (`0..99`, o `100..103` para `LFTC` 1..4) |
| `0x1C..0x23` | `0x45..0x4C` | 8 | `uint64` BE | **`EAN`** | Código de barras numérico (13-14 dígitos) |
| `0x24` | `0x4D` | 1 | `uint8` | `STRID1` | Message ID 1 (`0..99`) |
| `0x25` | `0x4E` | 1 | `uint8` | `STRID2` | Message ID 2 (`0..99`) |
| `0x26..0x27` | `0x4F..0x50` | 2 | `uint16` BE | **`DO1`** | Date Offset 1 en días (`0..999`) |
| `0x28` | `0x51` | 1 | `uint8` | **`DPC1`** | Date Print Control 1 (`0`=No imprimir, `1..4`) |
| `0x29..0x2A` | `0x52..0x53` | 2 | `uint16` BE | **`DO2`** | Date Offset 2 en días (`0..999`) |
| `0x2B` | `0x54` | 1 | `uint8` | **`DPC2`** | Date Print Control 2 (`0`=No imprimir, `1..4`) |
| `0x2C` | `0x55` | 1 | `uint8` | Flags precio/promo | `0x00` normal |
| `0x2D` | `0x56` | 1 | `uint8` | `TXID1` / `TXID2` | Nibble alto (`>> 4`) = `TXID1` (`0..9`), nibble bajo (`& 0x0F`) = `TXID2` (`0..9`) |
| `0x2E` | `0x57` | 1 | `uint8` | `STRIDPM` | Message ID promoción (`val >> 1`) |
| `0x2F` | `0x58` | 1 | `uint8` | `BPC` / `SOPLU` / `PMD` | `0x02` = normal (`BPC=0` imprime código de barras, `SOPLU=0`, `PMD=1`); `0x00` = suprime código de barras (`BPC=1`); `0x04` = `SOPLU=1` |
| `0x30` | `0x59` | 1 | `uint8` | `TPLU` / `TLKP` | Bit 7 = `TPLU`, Bit 6 = `TLKP` |
| `0x32..0x33` | `0x5B..0x5C` | 2 | `uint16` BE | Trace/Promo Ref | `0xFFFF` por defecto |
| `0x3F` | `0x68` | 1 | `uint8` | `IVID` | Information Voucher ID (`0..99`) |
| `0x40` | `0x69` | 1 | `uint8` | `TSID` | Traceability Scheme ID (`1..9`, default `1`) |
| `0x41` | `0x6A` | 1 | `uint8` | **`BID`** | Barcode Format ID (`0..32`) |
| `0x46` | `0x6F` | 1 | `uint8` | **`PT`** | Tipo de producto (ver tabla abajo) |
| `0x4F..0x50` | `0x78..0x79` | 2 | `uint8[2]` | Flags internos | Gestionados por la báscula (preservar al editar) |
| `0x51..0x52` | `0x7A..0x7B` | 2 | `uint16` | Hash interno | Calculado por la báscula (`00 00` al enviar) |

3. **Bytes `0x7C..fin`:** Cadenas CP1252 terminadas en `0x00`:
   `DYT\0`, `GPTA1\0`, `GPTA2\0`, `GPTA3\0`, `GPTA4\0`, `GPTA5\0`, `GPTA6\0` (terminado con `\0` final).

### Valores de `PT` (Tipo de Producto en `rec[0x46]`)

| Byte `rec[0x46]` | Código `xsp8000.dri` (`PT`) | Descripción |
|---:|---:|---|
| `0` | `0` | **Pesado** (*By Weight*, ej. por kg) |
| `1` | `1` (`PB=0`) | **Por pieza** (*By Count*) |
| `2` | `1` (`PB=1`) | **Por pieza negativo** (*Negative By Count*) |
| `3` | `4` | **Peso fijo** (*Fixed Weight*) |
| `4` | `2` | **Precio fijo** (*Fixed Price*) |
| `5` | `9` | **Peso manual** (*Manual Weight*) |
| `6` | `6` | **Por volumen** (*By Volume*) |

---

## 5. Otros Comandos Validados

### `CHANGE_PRICE` (`CMD = 5`, `ACTION = 'U'`, `CVN = 3400`) — 28 bytes
| Offset | Bytes | Tipo | Campo |
|---:|---:|---|---|
| `0..7` | 8 | `PluKey` BE | `FromKey = (DID << 56) | PLU` |
| `8..15` | 8 | `PluKey` BE | `ToKey = (DID << 56) | PLU` |
| `16..19` | 4 | `uint32` BE | `P1` (centavos) |
| `20..23` | 4 | `uint32` BE | `P2` (centavos) |
| `24..27` | 4 | `uint32` BE | `P3` (centavos) |

### `PLU:DELETEONE` (`CMD = 13`, `ACTION = 'C'`, `CVN = 7600`) — 41 bytes
Misma cabecera de 41 bytes que `PLU:READONE` (`FromKey = ToKey`, `bytes[39..40] = 00 63`).

### `VERSION_DETAILS` (`CMD = 59`, `ACTION = 'Q'`, `CVN = 0`) — 0 bytes
Devuelve cadenas separadas por `0x00`: `Product`, `Version`, `DateTime`, `Firmware`.

### `DATE_AND_TIME` (`CMD = 16`, `CVN = 11`)
- **Lectura (`ACTION = 'Q'`, payload vacío `0` bytes):** Devuelve 15 bytes: 6 enteros `uint16` BE (`day`, `month`, `year`, `hour`, `min`, `sec`) + 3 bytes finales.
- **Escritura (`ACTION = 'U'`, payload `13` bytes):**

| Offset | Bytes | Tipo | Campo |
|---:|---:|---|---|
| `0..1` | 2 | `uint16` BE | `day` (`1..31`) |
| `2..3` | 2 | `uint16` BE | `month` (`1..12`) |
| `4..5` | 2 | `uint16` BE | `year` (ej. `2026`) |
| `6..7` | 2 | `uint16` BE | `hour` (`0..23`) |
| `8..9` | 2 | `uint16` BE | `minute` (`0..59`) |
| `10..11` | 2 | `uint16` BE | `second` (`0..59`) |
| `12` | 1 | `uint8` | `0x00` |

### `ASSETINFO` (`CMD = 121` / `0x79`, `ACTION = 'Q'`, `CVN = 8000`) — 0 bytes
Devuelve **316 bytes** con información de activo, hardware y firmware (cadenas CP1252 terminadas en `0x00`):

| Offset | Bytes | Campo | Ejemplo validado (`192.168.9.113`) |
|---:|---:|---|---|
| `0x33..0x4A` | 24 | **Número de Serie** | `"22320310"` |
| `0x54..0x63` | 16 | Parte de Hardware | `"ABR25-001083"` |
| `0x69..0x72` | 10 | Modelo (`XS` + valor) | `"200"` (`XS200`) |
| `0x88..0x95` | 14 | Mercado / Región | `"GENERAL"` |
| `0x9D..0xB9` | 29 | Parte Firmware App | `"Xs Series ABR30-000403"` |
| `0xBC..0xCB` | 16 | Versión App | `"APP. v4.1.7.0"` |
| `0xCE..0xE0` | 19 | Fecha Compilación App | `"Jan 12 2016 10:05:21"` |
| `0xE1..0xF7` | 23 | Parte Firmware Boot | `"Xs Series ABR30-000402"` |
| `0x100..0x10F` | 16 | Versión Boot | `"BOOT v5.2.7.538"` |
| `0x110..0x124` | 21 | Fecha Compilación Boot | `"Nov 26 2021 18:13:51"` |
| `0x125..0x132` | 14 | Protocolo | `"PROTOCOL 8000"` |

### `MACHNETCONFIG` (`CMD = 128` / `0x80`, `ACTION = 'Q'`, `CVN = 8000`) — 0 bytes
Devuelve **48 bytes** con la configuración de red (`NETCONFIG:READONE`):

| Offset | Bytes | Tipo | Campo | Ejemplo validado |
|---:|---:|---|---|---|
| `0x00..0x03` | 4 | IPv4 | `IPAD` (IP) | `192.168.9.113` |
| `0x04..0x07` | 4 | IPv4 | `SNET` (Máscara) | `255.255.255.0` |
| `0x08..0x0B` | 4 | IPv4 | `GWAY` (Gateway) | `0.0.0.0` |
| `0x0C..0x0F` | 4 | IPv4 | `DNSS` (DNS) | `0.0.0.0` |
| `0x10..0x1F` | 16 | `char[16]` | `HNAME` (Hostname) | `""` |
| `0x20..0x25` | 6 | `byte[6]` | **`MAC`** | `00:E0:8A:0D:72:AF` |
| `0x26..0x27` | 2 | `uint16` BE | `SPORT` (Puerto inter-báscula) | `25000` (`0x61A8`) |
| `0x28..0x29` | 2 | `uint16` BE | `HPORT` (Puerto host TCP) | `3001` (`0x0BB9`) |
| `0x2A` | 1 | `uint8` | `DHCP` (`0`=Estática, `1`=DHCP) | `0` |
| `0x2B` | 1 | `uint8` | `WIFI` (`0`=Ethernet, `1`=Wi-Fi) | `0` |
| `0x2C` | 1 | `uint8` | `NETID` (`0..99`) | `0` |

### `BARCODE_FORMATS` (`CMD = 7`, `ACTION = 'Q'` / `'U'`, `CVN = 5000`)
- **Lectura (`'Q'`):** 1 byte `[BID]` (`1..32`).
- **Escritura (`'U'`):** `(6 + BFD.Length)` bytes con la **misma estructura exacta** que la respuesta (validado en `mos.dll` `MosBarcodeFormatP5000` y en vivo en `BID 32` con restauración):

| Offset | Bytes | Tipo | Campo | Valores / Decodificación |
|---:|---:|---|---|---|
| `0` | 1 | `uint8` | `BID` | ID del formato (`1..32`) |
| `1` | 1 | `uint8` | `BMC` (wire) | `0`=Etiqueta (`3`), `1`=Recibo (`1`), `2`=Talón (`2`), `3`=Vale Promocional (`4`) |
| `2` | 1 | `uint8` | `BT` (wire) | `0`=EAN-8, `1`=UPC-12, `2`=UPC-13, **`3`=EAN-13**, `5`=EAN-128, `6`=Code 39, `7`=Code 128, `8`=ITF-14, `9`=Ext. Code 39, `10`=PDF417 Text, `11`=PDF417 AI, `12`=GS1 DataBar Exp., `13`=GS1 DataBar 14, `14`=GS1 DataBar 14 Lim. |
| `3` | 1 | `uint8` | `BPD` | Divisor de precio (`1`, `10`, `100`) |
| `4` | 1 | `uint8` | Subtipo | `0x03` (añadido en `CVN = 5000`) |
| `5..fin` | var | `char[]` | `BFD` | Plantilla CP1252 terminada en `\0` (ej. `"2NNAAAAWWWWWC"`, `"02AAAAVPPPPPC"`) |

### `LABEL_FORMATS` (`CMD = 29`, `ACTION = 'Q'` / `'U'`, `CVN = 5200`)
- **Lectura (`'Q'`):** 2 bytes `[PFID, SEID=0]` (`PFID = 0..99`). Devuelve **898 bytes** en 2 bloques TCP.
- **Escritura (`'U'`):** **898 bytes** en 2 bloques TCP (500 B + 398 B) con la misma estructura exacta. Los 2 bytes finales de checksum (`896..897`) se envían en `00 00` y **la báscula los recalcula automáticamente** (validado en vivo clonando/editando `PFID 98`).
- **Limpieza / Eliminación:** `ACTION = 'C'` devuelve `-1117` (`ERR_UNEXPECTED_ACTION`); para vaciar un formato se envía `ACTION = 'U'` de 898 bytes con `Active = 0`, `FW = 0` y los 50 slots de campos en `0`.

| Offset | Bytes | Tipo | Campo | Descripción |
|---:|---:|---|---|---|
| `0x00` | 1 | `uint8` | `PFID` | ID de formato de etiqueta (`0..99`) |
| `0x01` | 1 | `uint8` | `FO` | Orientación: `0`=0°, `1`=90°, `2`=180°, `3`=270° |
| `0x02` | 1 | `uint8` | `Active` | `1`=Formato definido, `0`=Vacío (ej. `PFID 97, 98`) |
| `0x05..0x06` | 2 | `uint16` BE | `FW` | Ancho del formato en centésimas de mm (`5900` = 59.0 mm) |
| `0x08..0x23` | 28 | `char[28]` | Nombre | Descripción/nombre opcional (CP1252 o espacios) |
| `0x2D..0x37E` | 850 | `Field[50]` | Campos | 50 slots fijos de **17 bytes** cada uno (`FT=0` indica slot vacío) |
| `0x37F..0x381` | 3 | `byte[3]` | Cola | `0x00` + checksum interno de 2 bytes |

Cada registro de **campo (17 bytes)** tiene la estructura:
- `+0` (`uint8`): `FT` (Field Type: `1`=Texto 1, `2`=Texto 2, `3`=Texto 3, `5`/`6`=Fechas 1/2, `7`=Precio Unit. 1, `8`=Peso/Cantidad, `9`=Precio Total 1, `10`=Código de Barras, `11..13`=Leyendas, `14..18`=Símbolos/Tara, `21`=Precio Total 2, `53`=Logo, `57`/`58`=Línea/Caja, `142`=Nutricional)
- `+1` (`uint8`): `FDO` (Orientación del campo: `0..3` → 0°/90°/180°/270°)
- `+2` (`uint8`): Estilo/alineación (`'1'`, `'2'`, `'3'`)
- `+3..+10` (`4 × uint16` BE): `X1`, `Y1`, `X2`, `Y2` (en décimas de mm)
- `+11` (`char`): `FF` (Fuente ABML: `'0'..'8'`, `'A'..'X'`)
- `+12` (`uint8`): `DSR` (Referencia de tabla: para `FT=10` es el `BID`, para `FT=11..13` es el ID de mensaje, para `FT=53` es el ID de logo)

### Conteo Rápido de PLUs (Búsqueda Binaria en `SearchIndex`)
El campo `SearchIndex` (`bytes[32..35]`, `uint32` BE) de la petición `PLU:READ` (`CMD=13`, `ACTION='Q'`, `CVN=7600`) soporta **acceso aleatorio O(1)** dentro del rango `[FromKey .. ToKey]`:
- Para obtener el total exacto sin descargar todo el catálogo (ej. 821 PLUs ≈ 18 s secuenciales), se aplica búsqueda exponencial (`0, 1, 2, 4, 8, ...`) seguida de búsqueda binaria hasta encontrar el último índice que devuelve `Status = 0` (antes de `-1205` `PLU_LAST_PLU`).
- Costo: **O(log N)** (~15 paquetes, **~400 ms** para 821 PLUs).
- Para consultar **todos los departamentos**, usar `FromDept = 0, FromPLU = 1` y `ToDept = 99, ToPLU = 9999999999999`.

---

## 6. Códigos de Estado (`STATUS`)

| Código (`int16`) | Nombre | Significado |
|---:|---|---|
| `0` | `SUCCESS` | Operación exitosa |
| `-1108` | `ERR_INVALID_DATA` | Datos inválidos |
| `-1109` | `ERR_BAD_PARAMETER` | Parámetro inválido |
| `-1117` | `ERR_UNEXPECTED_ACTION` | Acción no válida para el comando |
| `-1119` | `ERR_BUSY` | Báscula ocupada |
| `-1141` | `ERR_UNSUPPORTED_CVN` | `CVN` no soportado por el firmware |
| `-1203` | `ERROR_PLU_NOT_FOUND` | PLU no encontrado |
| `-1205` | `PLU_LAST_PLU` | Fin de recorrido de PLUs |

---

## 7. Descubrimiento Automático en Red (UDP Broadcast / Unicast)

Extraído de `Asset Scanner.dll` (`Asset_Scanner.MSeries`) y validado en vivo contra dos básculas físicas **XS200** (`192.168.9.111` y `192.168.9.113`, respuestas en **20–70 ms**):

- **Protocolo:** UDP hacia `255.255.255.255:23452` (y broadcast de subred `192.168.x.255:23452`; o unicast `<IP>:23452` para escaneo por rango en redes enrutadas).
- **Payload ASCII (16 bytes):** `:AVERYBERKELENQ:`
- **Respuesta real de la báscula (texto ASCII, 112 bytes):**
  ```text
  Hello from X LPC1833 [ Xs Series ABR30-000403 APP. v4.1.7.0 ]  Scale Port = 25000  (192.168.9.113)  {ASSET_ID}
  ```
- **Campos parseados:** Rol (`S`=Server, `C`=Client, `E`=Standalone, `X`=XS), plataforma (`LPC1833`), modelo/familia (`Xs Series`), versión (`APP. v4.1.7.0`), puerto inter-báscula (`25000`), e IP.
- **Enriquecimiento opcional:** Con una sola sesión TCP a `<IP>:3001` (`ASSETINFO` + `MACHNETCONFIG`) se agregan **Modelo exacto (`XS200`)**, **Número de Serie** (`22320310` / `24310206`) y **MAC** (`00:E0:8A:...`).

---

## 8. Configuración de Báscula y Tablas Maestras (Validado en Vivo)

### `MACHINE_PARAMETERS` (`CMD = 24`, `CVN = 8000`, `ACTION = 'Q'` / `'U'`) — 80 bytes
Controla los modos de operación, impresión y hardware de la báscula (extraído de `XsMachineParamP8000` en `mos.dll`, validado lectura y escritura idempotente en `192.168.9.113:3001`):

| Offset | Campo (`xsp8000.dri`) | Descripción y Valores |
|---:|---|---|
| `0x04` | `LFTP1` | Tipo de formato Impresora 1 (`0`=Cerrado, `1`=Abierto) |
| `0x05` | `PFIDDP1` | Formato de etiqueta por defecto Impresora 1 (`0..99`) |
| `0x06` | `LFTP2` | Tipo de formato Impresora 2 (`0`=Cerrado, `1`=Abierto) |
| `0x07` | `PFIDDP2` | Formato de etiqueta por defecto Impresora 2 (`0..99`) |
| `0x08` | `LTYCSM` | Tipo de etiqueta en mostrador (`0`=Separada, `1`=Continua) |
| `0x09` | **`PTM`** | **Modo de Impresión / Operación:** `0`=Bajo demanda/Mostrador (*on request*), `1`=Inmediato (*immediate*), `2`=Pre-empaque (*pre-pack*) |
| `0x0C` | `BIDLD` | Formato de código de barras por defecto para etiquetas (`0..32`) |
| `0x13` | `LDT` | Sensor de etiqueta / Label Detect (`0`=No, `1`=Sí) |
| `0x14` | `FT` | Tara forzada (`0`=Sin restricción, `1`=Siempre requerida, `2`=Manual no sobrescribible) |
| `0x18` | **`CKMD`** | **Modo Caja / Checkout:** `0`=No, `1`=Sí, `2`=Sí sin fondo |
| `0x19` | **`POSM`** | **Modo PoS:** `0`=No, `1`=Sí |
| `0x1C` | `NEA` | Alarma de error de red (`0`=No, `1`=Sí) |
| `0x1F` | `PFIDPL` | Formato de etiqueta Passport (`90` por defecto) |
| `0x20` | **`SSM`** | **Modo Autoservicio (*Self Service Mode*):** `0`=Deshabilitado, `1`=Habilitado |
| `0x25` | `P2OM` | Modo Impresora 2 (`0`=Off, `1`=Recibo/Reportes, `2`=Etiqueta, `3`=Todos) |
| `0x28` | **`OSLM`** | **Etiqueta de un solo disparo (*One Shot Label Mode*):** `0`=No, `1`=Sí |
| `0x30` | `LPS` | Velocidad de impresión de etiquetas (`30..100`%) |
| `0x32` | `RPS` | Velocidad de impresión de recibos (`30..100`%) |
| `0x34` | `PPS` | Velocidad de impresión Impresora 2 (`30..100`%) |
| `0x4B` | `RBH` | Altura código de barras en recibo |
| `0x4D` | `LTMC` | Mensaje "Retire etiqueta" (`0`=Activo, `1`=Deshabilitado) |
| `0x4E` | `TBRF` | Barra de corte (*Tear Bar Fitted*: `0`=No, `1`=Sí) |

> Al modificar cualquier parámetro con `CMD=24, ACTION='U', CVN=8000`, leer primero los 80 bytes actuales, modificar únicamente los offsets deseados y reenviar los 80 bytes completos para preservar el resto de la configuración.

### Otras Tablas Maestras Validadas en Vivo (`192.168.9.113:3001`)

| Comando | `CMD` | `CVN` | Lectura (`'Q'`) | Escritura (`'U'`) | Respuesta validada |
|---|---:|---:|---|---|---|
| **Departamentos** (`DEPT_NAMES`) | `39` | `11` | 1 B `[DID]` (`0..99`) | `(2+len)` B `[DID]` + Nombre CP1252 + `\0` | 24 B: `byte[0]=DID`, `bytes[1..]=Nombre` CP1252 (ej. `DID 0="PRODUCTOS EL 100"`, `DID 1="GENERAL"`) |
| **Taras / Contenedores** (`STORED_TARE`) | `19` | `5000` | **5 B** `[0,0,0,0,CID]` (`0..39`) | **5 B** `[CTV (uint32 BE gramos), CID]` | 8 B: `bytes[0..3]=CTV` (gramos, `uint32` BE), `byte[4]=CID` |
| **Impuestos** (`TAX_RATES`) | `20` | `11` | **5 B** `[0,0,0,0,TXID]` (`0..9`) | **5 B** `[TR (uint32 BE centésimas %), TXID]` | 8 B: `bytes[0..3]=TR` (`1600` = `16.00%`, validado en `TXID 1`), `byte[4]=TXID` |
| **Logos / Media** (`MEDIA`) | `37` | `7200` | 1 B `[MEID]` (`0..99`) | — | Nombre (offset `2..31`) + imagen **BMP** nativa desde offset `33` (`"BM"`, ej. `MEID 0="calimax"`, `MEID 1="blanquita bm"`) |
| **Textos / Leyendas** (`STORED_TEXT`) | `6` | `11` | 2 B `[ID, Tabla]` | `(2+len)` B | `byte[0]=ID`, `bytes[1..]=Texto` CP1252 (ej. `MSG 1="Fecha De Venta:"`, `MSG 3="CARNICERIA LA BLANQUITA"`) |
| **Parámetros de Red** (`NETWORK_PARAMETERS`) | `86` | `7200` | 0 B | — | 13 B de configuración de red/reportes |

### Nota sobre `Scale Settings` (`#748` `SS:READONE`) y `Diagnostic Logs` (`#747`)
Los ajustes de configuración profunda de calibración/región (`DSM`, `PM`, `CAPF`, `DPPF`, `CSY`, `LNR`, `CRE`, `TS` en `XsScaleSettingsP8000`) y los logs de diagnóstico (`#747`) **no viajan por `MSeriesProtocol` (TCP `3001`)**, sino por el puerto de servicio **`XsServicePort` (TCP `28822`)** iniciando sesión con el comando de texto `:CONNECT:`. Para el día a día de operación comercial (modo de impresión/pre-empaque/autoservicio/PoS/checkout, velocidades, formatos por defecto, PLUs, precios, reloj, etiquetas y códigos de barras), todo está cubierto directamente sobre **TCP `3001`**.

---

## 9. Lectura de Totales y Ventas (Validado en Vivo)

Todas las consultas de totales usan `ACTION = 'Q'` (solo lectura) y soportan **2 períodos acumulados** (`Period = 0` actual, `Period = 1` período 2). Si un registro no tiene ventas acumuladas, la báscula devuelve `STATUS = -1125` (`ERR_NO_TOTALS_FOUND`).

| Comando | `CMD` | `CVN` | Petición (`'Q'`) | Respuesta | Offsets clave (Big-Endian) |
|---|---:|---:|---|---|---|
| **`GRAND_TOTALS`** | `10` | `11` | 1 B `[Period]` | `169 B` | `0x09..0x10` Mostrador (`int64`, centavos), `0x11..0x18` Pre-empaque (`int64`), `0x19..0x1C` Transacciones (`uint32`), `0x4D..0x54` Total Bruto (`int64`), `0x9D..0xA8` Fecha último corte (`6×uint16`) |
| **`DEPT_TOTALS`** | `41` | `11` | 3 B `[Period, StartDID, 1]` (busca siguiente depto con ventas ≥ `StartDID`) | `87 B` | `0x01` `DID` encontrado, `0x0B..0x12` Mostrador (`int64`), `0x13..0x1A` Pre-empaque (`int64`), `0x1B..0x1E` Transacciones (`uint32`), últimos 12 B = Fecha último corte |
| **`GROUP_TOTALS`** | `74` | `11` | 3 B `[Period, GID, 0]` | `83 B` | `0x0B..0x12` Mostrador, `0x13..0x1A` Pre-empaque, `0x1B..0x1E` Transacciones, `0x48..0x52` Fecha último corte |
| **`OPERATOR_TOTALS`** | `8` | `11` | 3 B `[Period, OPID, 0]` | `311 B` | `0x0B..0x12` Mostrador, `0x13..0x1A` Pre-empaque, `0x1F..0x22` Transacciones |
| **`MACH_TOTALS`** | `43` | `11` | 1 B `[Period]` | `303 B` | Mismos acumulados globales de la máquina |
| **`PLU_TOTALS`** | `9` | `7600` | **42 B** (`[Period]` + cabecera búsqueda 41 B con `SearchIndex` en `33..36`) | `166 B` | `42..49` `PluKey`, `50..53` Transacciones (`uint32`), `54..61` Mostrador (`int64`, centavos), `62..65` Cantidad/Peso Mostrador (`uint32`, g o pz), `66..69` Tx Mostrador, `70..77` Pre-empaque (`int64`), `78..81` Cant. Pre-empaque, `0x9A..0xA5` Fecha último corte |

- **Conteo rápido O(log N) en `PLU_TOTALS`:** igual que `PLU_RECORD`, soporta búsqueda exponencial + binaria sobre `SearchIndex` (`bytes[33..36]`), obteniendo el total exacto de PLUs con ventas (**789 PLUs** en `192.168.9.113`) en **~288 ms**.
- **Validación cruzada en vivo (`192.168.9.113`):** `PLU 1` (`ALITA DE POLLO ADOBADA` a \$70.00/kg) reportó `34` transacciones, `18.640 kg` y `\$1,304.80` (`18.640 × 70.00 = 1,304.80` exacto). `GRAND_TOTALS` y `DEPT_TOTALS(DID=1)` coincidieron al centavo (`\$89,864,332.29` / `635,636` transacciones).

---

## 10. Puerto de Servicio `XsServicePort` (TCP `28822` — Validado en Vivo)

Extraído de `XsServicePort.class` / `XsServicePortCommand.class` (`ScaleDriver.jar`) y validado en vivo contra **`192.168.9.113:28822`**:

### Enmarcado y Cifrado de Flujo Simétrico (`XsServicePortCommand$Crypt`)
- **Transporte:** TCP `28822`, texto `ISO-8859-1` delimitado por **`STX` (`0x02`)** y **`ETX` (`0x03`)**.
- **Petición en texto plano:** `\x02SelectCommand:<N>:<payload>\x03`
- **Respuesta en texto plano:** `\x02ReplyCommand:<NN>:<datos>\x03` o `\x02CommandError:<NN>:<error>\x03`
- **Cifrado simétrico por mensaje:** Cada mensaje enviado y cada respuesta leída reinicia el estado de 10 bytes `state` con la clave fija (`signed byte[]`):
  `{ -52, -74, -89, -9, -53, -56, 105, -51, -50, -49 }` (`CC B6 A7 F7 CB C8 69 CD CE CF`), `ptr = 0`.
  Para cada byte `b`:
  ```csharp
  b ^= (sbyte)(state[ptr] ^ (state[0] * ptr));
  int next = (ptr < 9) ? (ptr + 1) : 0;
  state[ptr] = (sbyte)(state[ptr] + state[next]);
  if (state[ptr] == 0) state[ptr] = 1;
  ptr = (ptr + 1) % 10;
  ```

### Comandos Principales de `XsServicePort` (63 comandos totales)

| `CMD` | Nombre | Payload | Respuesta validada en `192.168.9.113:28822` |
|---:|---|---|---|
| `1` / `2` | `READ/WRITE_SERIAL_NUMBER` | — | `22320310` |
| `3` / `4` | `READ/WRITE_MODEL_NUMBER` | — | `200` (`XS200`) |
| `5` / `7` | `READ_GRAVITY_FACTOR_FACTORY` / `_SITE` | — | `981287` (`9.81287 m/s²`) |
| `9` / `10` | `READ/WRITE_CAPACITY` | — | `15kg x 5g` |
| `14` | **`READ_WEIGHT`** (peso en vivo) | — | `0.000,04/08,0.500,Balance,` |
| `15` / `16` / `17` | `BALANCE_SCALE` / `WRITE_TARE` / `READ_TARE` | — | `0.000,04/08,0.500,Balance,` |
| `18`..`25` | `READ/WRITE_IP`, `SUBNET`, `GATEWAY`, `WIRED_MAC`, `ENABLE_DHCP` | — | `192.168.009.113`, `255.255.255.000`, `00:E0:8A:0D:72:AF` |
| `30` | `READ_POOL_SIZE` | `PLUMEM` / `GENMEM` / `DIAGLOG` / `SYSTEM` | Tamaño y memoria libre del pool |
| `31` | `READ_WIRELESS_MAC` | — | `NO WIRELESS FITTED` |
| `32` / `33` / `34` | `READ_SCALE_BOOT_VERSION` / `_APP_VERSION` / `_VAL_LIB_VERSION` | — | `Xs Series ABR30-000403,v4.1.7.0,PROTOCOL 8000` |
| `35` / `36` | `READ/WRITE_DATETIME` | `dd:MM:yyyy HH:mm:ss` | `22:11:2026 15:11:52` |
| `40` | `READ_DISPLAY_VERSION` | `VENDOR` / `CUSTOMER` | Versión App/Boot de los displays |
| `42` | `READ_MEMORY_DETAILS` | `PLU`/`POOL`/`TRANS`/`AUDIT` | Bloques usados/libres y conteo PLU |
| `46` / `47` | `ACTIVATE_CASH_DRAWER` / `PRINT_TEST_REPORT` | — | Apertura cajón / impresión de prueba |
| `52` | `READ_DIAGNOSTIC_LOG` | `<SYSTEM\|COMMS\|KEYBOARD>,<0\|1>` | Multi-trama (`1,N,...` hasta `N,N,...`): leer todas las tramas hasta `cur == tot` (`0`=no borrar) |
| `53` | `REBOOT` | — | Reinicio remoto de la báscula |
| `55` | `READ_OPERATING_MODE` | — | `APP` (o `BOOT`) |
| `56` | **`READ_DISPLAY_BITMAP`** | — | Captura en vivo LCD **`320 x 128,1,<10240 hex>`** (1 bpp) |
| `57` | `WRITE_KEYSTROKE` | `<scanCode>,<keyState>` | Simulación remota de teclas |
| `62` / `63` | **`READ/WRITE_CONFIGURATION`** (`#748 Scale Settings`) | `ver,b1,b2,b3,b4` | `0001,8610011020214130,0020020110111000,0110110100111000,0100130010003200` (4 bancos × 16 switches: `CAPF=8` 15kg×5g, `PM=2` Etiqueta, `DSM=1` PLU Pesado, `DPPF=2`, `CSY=02` \$, `LNR=01` Español, `CRE=32` México) |

- Cliente interactivo listo para usar: [xs_service_port.bat](file:///c:/Users/Alexis/Desktop/Desarrollo/AveryBerkel/xs_service_port.bat) + [scripts/xs_service_port.ps1](file:///c:/Users/Alexis/Desktop/Desarrollo/AveryBerkel/scripts/xs_service_port.ps1).
