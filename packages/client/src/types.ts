export type ScaleFamily = "avery_berkel_xs" | "avery_berkel_xti";

export type ScaleStatus = "unknown" | "online" | "offline" | "syncing" | "error";

export type ProductType =
  | "by_weight"
  | "by_count"
  | "negative_by_count"
  | "fixed_weight"
  | "fixed_price"
  | "manual_weight"
  | "by_volume";

export type TareMode = "table_reference" | "direct_grams";

export type Role = "viewer" | "operator" | "admin" | "technician";

export interface PluKey {
  did: number;
  plu: number;
}

export interface SyncMetadata {
  store_id: string;
  sync_version: number;
  updated_at: string;
  deleted_at: string | null;
}

export interface Scale {
  id: string;
  group_id: string | null;
  name: string;
  family: ScaleFamily;
  ip_address: string;
  data_port: number;
  service_port: number | null;
  serial_number: string | null;
  model: string | null;
  firmware_version: string | null;
  mac_address: string | null;
  status: ScaleStatus;
  last_seen_at: string | null;
  meta: SyncMetadata;
}

export interface DiscoveredScale {
  family: ScaleFamily;
  ip_address: string;
  data_port: number;
  service_port: number | null;
  model: string | null;
  serial_number: string | null;
  mac_address: string | null;
  firmware_version: string | null;
}

export interface DriverCapabilities {
  supports_fast_price_change: boolean;
  supports_service_port: boolean;
  supports_live_weight: boolean;
  supports_lcd_mirror: boolean;
  supports_remote_keys: boolean;
  supports_sql_filter: boolean;
  did_range: { min: number; max: number };
  plu_max_digits: number;
  text_areas_count: number;
  tare_mode: TareMode;
}

export interface Plu {
  id: string;
  key: PluKey;
  description: string;
  button_text: string | null;
  price_1: number;
  price_2: number;
  price_3: number;
  product_type: ProductType;
  ean: string | null;
  sku: string | null;
  group_id: number;
  barcode_format_id: number;
  label_format_id: number;
  tare_id: number | null;
  tare_grams: number | null;
  date_offset_1: number;
  date_print_control_1: number;
  date_offset_2: number;
  date_print_control_2: number;
  tax_rate_id_1: number;
  tax_rate_id_2: number;
  text_areas: string[];
  delta_hash: string;
  meta: SyncMetadata;
}

export interface LiveWeight {
  gross_grams: number;
  tare_grams: number;
  net_grams: number;
  is_stable: boolean;
  is_zero: boolean;
  captured_at: string;
}

export type ScaleEvent =
  | {
      type: "connection_changed";
      scale_id: string;
      state: "disconnected" | "connecting" | "connected" | "reconnecting" | "error";
      message?: string | null;
    }
  | {
      type: "live_weight_updated";
      scale_id: string;
      weight: LiveWeight;
    };
