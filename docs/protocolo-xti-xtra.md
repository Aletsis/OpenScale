# Protocolo de Comunicación Avery Berkel — Serie XTi (`XtraDataPort` / `XTRA`)

> **Estado:** Especificación extraída y verificada a nivel de bytecode/binarios oficiales (`ScaleDriver.jar`, `MxiComponents.jar`, `DataValidator.jar`, `xtra.dll`, `XMP5.3.0.0.DRI`) y probada con servidor mock TCP local. Pendiente validación final contra una báscula XTi física en red.  
> **Aplica a:** `XT Series` (**XTi**, `XTs`, `XTx`), `XTB Series` y `XM Series` (todas comparten la familia `XTRA`).

---

## 1. Capa de Transporte y Primitivas Binarias

| Parámetro | Valor |
|---|---|
| Transporte | **TCP/IP** (`KeepAlive = true`) |
| Puerto de datos | **`3002`** (`XtraDataPort`) |
| Puerto de descubrimiento LAN | **UDP `23453`** (`:AVERYBERKELXTRAENQ:`) |
| Codificación de longitudes / contadores | **Entero sin signo de 64 bits Little-Endian (`uint64` LE, 8 bytes)** |
| Codificación de texto | **UTF-8** (`StandardCharsets.UTF_8`) |
| Identificador de éxito | `STATUS = "0"` en la metadata de la respuesta |

### Primitivas del stream (`MxApiOutputStream` / `MxApiInputStream`)

1. **`uint64 LE` (8 bytes):** Todos los contadores y tamaños de bloque se escriben en 8 bytes Little-Endian.
2. **`Bytes` / `String`:**
   - Si es `null` o vacío sin valor: `00 00 00 00 00 00 00 00` (8 bytes en `0`).
   - Si contiene datos: `[uint64 LE byteLen][bytes UTF-8 o binarios]`.
3. **`Map` (pares clave/valor):**
   ```text
   [uint64 LE numPairs]
     Repetido numPairs veces:
       [KeyString:   uint64 LE keyLen   + UTF-8 bytes]
       [ValueBytes:  uint64 LE valLen   + UTF-8 / raw bytes]
   ```

---

## 2. Handshake Inicial: Banner XML (Báscula → Cliente)

A diferencia de la serie XS (donde el cliente envía primero `VERSION_DETAILS`), al abrir el socket TCP en **`<IP>:3002`**, **la báscula XTi envía inmediatamente su Banner XML**:

```text
[uint64 LE xmlByteLength][UTF-8 XML]
```

Ejemplo de estructura XML procesada por `XtraDataPortDriver.readBanner()` y `xtra.dll`:

```xml
<Banner>
  <Field name="ScaleType">XT</Field>
  <Field name="ModelNumber">XTi400</Field>
  <Field name="SerialNumber">12345678</Field>
  <Field name="ScaleVersion">5.3.0.0</Field>
  <Field name="Schema Number">5300</Field>
  <Field name="Native Schema Number">5300</Field>
  <Field name="FirmwareNumber">ABR30-000341</Field>
  <Field name="MACAddress">00:11:22:33:44:55</Field>
  <Field name="DHCP">No</Field>
  <Field name="Authenticate">No</Field>
</Banner>
```

- **`FirmwareNumber` conocidos:** `"ABR30-000341"` = **XT / XTi Series**; `"ABR30-000413"` = **XTB Series**.
- **`Authenticate`:** Por defecto `"No"`. Si una báscula XTi tiene habilitada seguridad de acceso en red (`"Yes"`), requerirá flujo de autenticación antes de aceptar comandos de escritura.

---

## 3. Formato de Petición (`Instruction`: Cliente → Báscula)

Cada comando enviado por el cliente (`MxApiOutputStream.print(Record)`) tiene exactamente **2 bloques consecutivos**:

```text
+---------------------------------------------------------------+
| 1. Metadata Map (sin prefijo de longitud total)               |
|    [uint64 LE numMetaPairs]  (normalmente 2)                  |
|    [Key="TABLE_ID"]  [Val="PLU"]                              |
|    [Key="ACTION_ID"] [Val="WRITE" | "READ" | "READONE" | ...] |
+---------------------------------------------------------------+
| 2. Payload (envuelto con longitud total en bytes!)            |
|    [uint64 LE totalInnerPayloadBytes]                         |
|      +-----------------------------------------------------+  |
|      | Inner Payload Map                                   |  |
|      | [uint64 LE numFields]                               |  |
|      | [Key="DID"]  [Val="1"]                              |  |
|      | [Key="PLU"]  [Val="100"]                            |  |
|      | [Key="DYT"]  [Val="POLLO FRESCO"]                   |  |
|      | [Key="P1"]   [Val="4500"]                           |  |
|      | ...                                                 |  |
|      +-----------------------------------------------------+  |
+---------------------------------------------------------------+
```

> **Detalle clave de serialización:** El bloque `Payload` saliente lleva un prefijo **`uint64 LE` con el tamaño total en bytes** del mapa interno (contando los 8 bytes de `numFields` + todos los pares `[len+key][len+val]`). Si no hay campos de payload (ej. `PING:READ`), el mapa interno son 8 bytes en `0` (`numFields = 0`), por lo que `totalInnerPayloadBytes = 8`.

---

## 4. Formato de Respuesta (`ResponseBatch`: Báscula → Cliente)

Leído por `MxApiInputStream.read(ResponseBatch, true)` con **`m_bDirectToScale = true`**:

```text
+---------------------------------------------------------------+
| 1. Batch Metadata Map                                         |
|    [uint64 LE numMetaPairs]                                   |
|    [Key="STATUS"]  [Val="0"]  ("0" = OK; otro valor = error)  |
|    [Key="MESSAGE"] [Val="..."] (opcional en errores)          |
+---------------------------------------------------------------+
| 2. Record Count                                               |
|    [uint64 LE numRecords]                                     |
+---------------------------------------------------------------+
| 3. Repetido numRecords veces (SIN metadata por registro y     |
|    SIN prefijo extra de longitud de payload):                 |
|    [uint64 LE numFields]                                      |
|    [Key=FieldName] [Val=FieldValue] ...                       |
+---------------------------------------------------------------+
```

- Si el nombre de un campo comienza con `"BLOB"` el valor son bytes crudos (ej. imágenes); si comienza con `"CLOB"` es texto XML; el resto son cadenas UTF-8.

---

## 5. Catálogo de Comandos PLU (`XMP5.3.0.0.DRI`)

| Operación | `TABLE_ID` | `ACTION_ID` | Campos clave de entrada (`Payload`) |
|---|---|---|---|
| **Ping / Estado** | `PING` | `READ` | *(vacío)* → devuelve `CCS`, `CLOB` |
| **Leer 1 PLU** | `PLU` | `READONE` | `F1` (Departamento, `1..999999`), `F2` (Número PLU, `1..99999999999999`) |
| **Listar PLUs** | `PLU` | `READ` | `F1` (FromDept), `F2` (FromPLU), `T1` (ToDept), `T2` (ToPLU), opcional `SQLW` (cláusula WHERE SQL) |
| **Crear / Actualizar PLU** | `PLU` | `WRITE` | `DID`, `PLU`, `DYT`, `P1`, `P2FS`, `PT`, `EAN`, `BID`, `PFID`, `DO1`, `DPC1`, `DO2`, `DPC2`, `CTV`, `LGID`, `TXID1`, `GPTA1..12`, etc. |
| **Cambio de Precio** | `PLU` | `WRITE` | En XTi no existe `PLUCHG` separado: se lee el PLU con `READONE` (para no perder atributos) y se reenvía con `PLU:WRITE` actualizando `P1`/`P2FS`. |
| **Eliminar 1 PLU** | `PLU` | `DELETEONE` | `F1` (Departamento), `F2` (PLU) *(también acepta `DID`/`PLU` según `KeyFieldTransformer`)* |
| **Eliminar por Depto** | `PLU` | `DELETE` | `F1` (FromDept), `PDDC` (`0`/`1`) |

### Campos principales de `PLU` en XTi (`XMP5.3.0.0.DRI`)

| Campo | Tipo | Rango / Longitud | Descripción |
|---|---|---|---|
| `DID` | Numeric | `1..999999` | Department ID *(en XTi inicia en `1`, no `0`)* |
| `PLU` | Numeric | `1..99999999999999` | Número de PLU (hasta 14 dígitos) |
| `DYT` | String | hasta 2000 chars | Descripción / Display Text |
| `DBT` | String | hasta 250 chars | Texto del botón táctil (*Display Button Text*) |
| `P1` | UnitValue | `0..99999999` | Precio 1 en centavos (`4500` = \$45.00) |
| `P2FS` | UnitValue | `0..99999999` | Precio 2 / Precio alternativo |
| `PT` | Flag | `0, 1, 4, 6` | `0`=Pesado, `1`=Por pieza, `4`=Peso fijo, `6`=Volumen |
| `EAN` | String | hasta 14 chars | Código de barras EAN (cadena de texto) |
| `SKU` | String | hasta 14 chars | Clave personalizada de artículo |
| `BID` | Numeric | `0..32` | Barcode Format ID |
| `PFID` | Numeric | `0..120` | Print Format ID (formato de etiqueta) |
| `CTV` | Weight | `0..300000` | Tara directa en gramos (*Container Tare Value*) |
| `DO1` / `DO2` / `DO3` | Numeric | `0..999` | Días de caducidad 1, 2 y 3 |
| `DPC1` / `DPC2` / `DPC3` | Flag | `0, 1, 2, 3` | Control de impresión de fechas |
| `LGID` | Numeric | `0..999999` | Group ID |
| `TXID1` / `TXID2` | Numeric | `0..9` | Tax Rate ID primario y secundario |
| `GPTA1` .. `GPTA12` | Text | hasta 2500 chars c/u | Áreas de texto (etiqueta, ingredientes, alérgenos, etc.) |

---

## 6. Comparativa Directa: XS (puerto `3001`) vs. XTi (puerto `3002`)

Esta tabla resume lo esencial para diseñar una **capa de abstracción unificada** en tu futura alternativa a MXBusiness:

| Aspecto | **XS Series (`MSeriesProtocol`)** | **XTi Series (`XtraDataPort`)** |
|---|---|---|
| **Puerto TCP** | `3001` | `3002` |
| **Descubrimiento LAN (UDP)** | UDP `23452` (`:AVERYBERKELENQ:`) | UDP `23453` (`:AVERYBERKELXTRAENQ:`) |
| **Autodetección** | Cliente envía `VERSION_DETAILS` (`CMD=59`) | Báscula envía **Banner XML** automáticamente al conectar |
| **Formato de trama** | Binario fijo (cabecera 51 B + structs fijos + bloques max 500 B) | Stream dinámico (`uint64 LE` + pares clave/valor UTF-8) |
| **Endianness** | **Big-Endian** | **Little-Endian** (longitudes de 8 bytes) |
| **Texto** | `Windows-1252` (CP1252), terminado en `0x00` | **UTF-8**, prefijado con longitud |
| **Regla TCP crítica** | Cabecera + payload en 1 solo `Write()` | Stream estándar TCP |
| **Rango `DID` / `PLU`** | `DID`: `0..99`, `PLU`: 13 dígitos | `DID`: `1..999999`, `PLU`: 14 dígitos |
| **Cambio rápido precio** | Comando dedicado `CHANGE_PRICE` (`CMD=5, CVN=3400`, 28 B) | `PLU:WRITE` (leer + actualizar `P1`) |
| **Tara** | Referencia por tabla (`CID` `0..39`) | Valor directo (`CTV` en gramos) |
| **Áreas de texto** | `GPTA1..GPTA6` | `GPTA1..GPTA12` + `DBT` (botón táctil) |
| **Búsqueda filtrada** | Por rango (`FromKey`..`ToKey`) iterando `SearchIndex` | Rango (`F1/F2`..`T1/T2`) y cláusula **`SQLW`** |

---

## 7. Descubrimiento Automático en Red (UDP `23453`)

Extraído de `Asset Scanner.dll` (`Asset_Scanner.XMSeries`):

- **Protocolo:** UDP hacia `255.255.255.255:23453` (o unicast `<IP>:23453` para escaneo por rango).
- **Payload ASCII (20 bytes):** `:AVERYBERKELXTRAENQ:`
- **Respuesta XML de la báscula:**
  ```xml
  <Response>
    <Field name="ScaleType">XT</Field>
    <Field name="ModelNumber">XTi400</Field>
    <Field name="SerialNumber">12345678</Field>
    <Field name="IPAddress">192.168.9.120</Field>
    <Field name="MACAddress">00:E0:8A:...</Field>
    <Field name="HostPort">3002</Field>
    <Field name="ScaleVersion">5.3.0.0</Field>
    <Field name="ISCRole">standalone</Field>
  </Response>
  ```
- Implementado junto con XS en [scripts/discover_scales.ps1](file:///c:/Users/Alexis/Desktop/Desarrollo/AveryBerkel/scripts/discover_scales.ps1) y [descubrir_basculas.bat](file:///c:/Users/Alexis/Desktop/Desarrollo/AveryBerkel/descubrir_basculas.bat).
