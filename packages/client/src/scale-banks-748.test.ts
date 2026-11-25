import { describe, expect, it } from "vitest";
import {
  BANK_748_FIELDS,
  DEFAULT_BANKS_748,
  diffBanks748,
  readBankFieldValue,
  writeBankFieldValue,
} from "./scale-banks-748.js";

describe("scale-banks-748", () => {
  it("reads and updates verified parameters without mutating other digits", () => {
    const csy = BANK_748_FIELDS.find((f) => f.code === "CSY");
    const cre = BANK_748_FIELDS.find((f) => f.code === "CRE");
    if (!csy || !cre) {
      throw new Error("Expected CSY and CRE fields in BANK_748_FIELDS");
    }

    expect(readBankFieldValue(DEFAULT_BANKS_748, csy)).toBe("02");
    expect(readBankFieldValue(DEFAULT_BANKS_748, cre)).toBe("32");

    const updated = writeBankFieldValue(DEFAULT_BANKS_748, csy, "03");
    expect(readBankFieldValue(updated, csy)).toBe("03");
    expect(updated.bank1).toBe(DEFAULT_BANKS_748.bank1);
    expect(updated.bank3).toBe(DEFAULT_BANKS_748.bank3);
    expect(updated.bank4).toBe(DEFAULT_BANKS_748.bank4);

    const diffs = diffBanks748(DEFAULT_BANKS_748, updated);
    expect(diffs).toHaveLength(1);
    expect(diffs[0]?.field.code).toBe("CSY");
    expect(diffs[0]?.before).toBe("02");
    expect(diffs[0]?.after).toBe("03");
  });

  it("writes fields across banks 1, 3, and 4 without mutating sibling banks", () => {
    const bank1Field = BANK_748_FIELDS.find((f) => f.bank === 1);
    const bank3Field = BANK_748_FIELDS.find((f) => f.bank === 3);
    const bank4Field = BANK_748_FIELDS.find((f) => f.bank === 4);
    if (!bank1Field || !bank3Field || !bank4Field) {
      throw new Error("Expected fields in banks 1, 3, and 4");
    }

    const u1 = writeBankFieldValue(DEFAULT_BANKS_748, bank1Field, "1");
    expect(u1.bank2).toBe(DEFAULT_BANKS_748.bank2);
    const u3 = writeBankFieldValue(u1, bank3Field, "1");
    expect(u3.bank4).toBe(DEFAULT_BANKS_748.bank4);
    const u4 = writeBankFieldValue(u3, bank4Field, "1");
    expect(readBankFieldValue(u4, bank4Field)).toHaveLength(bank4Field.length);
  });
});
