import "@testing-library/jest-dom/vitest";
import type { OpenScaleTransport } from "@openscale/client";
import type { ColumnDef } from "@tanstack/react-table";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { OpenScaleProvider, useOpenScaleTransport } from "../providers/OpenScaleProvider.js";
import { Badge } from "./badge.js";
import { Button } from "./button.js";
import { DataTable } from "./data-table.js";

interface RowItem {
  plu: number;
  description: string;
}

const columns: ColumnDef<RowItem>[] = [
  { accessorKey: "plu", header: "PLU" },
  { accessorKey: "description", header: "Description" },
];

function TransportConsumer() {
  const transport = useOpenScaleTransport();
  return <div data-testid="has-transport">{typeof transport.listPlus}</div>;
}

describe("@openscale/ui components", () => {
  it("renders Button with variants and sizes", () => {
    render(
      <Button variant="destructive" size="sm" disabled>
        Delete
      </Button>,
    );
    const btn = screen.getByRole("button", { name: "Delete" });
    expect(btn).toBeDisabled();
    expect(btn.className).toContain("bg-destructive");
    expect(btn.className).toContain("h-8");
  });

  it("renders Badge with status variants", () => {
    render(<Badge variant="success">Online</Badge>);
    const badge = screen.getByText("Online");
    expect(badge.className).toContain("text-emerald-700");
  });

  it("renders DataTable rows and empty state", () => {
    const { rerender } = render(
      <DataTable
        columns={columns}
        data={[{ plu: 100, description: "POLLO FRESCO" }]}
        emptyMessage="Empty catalog"
      />,
    );
    expect(screen.getByText("100")).toBeInTheDocument();
    expect(screen.getByText("POLLO FRESCO")).toBeInTheDocument();

    rerender(<DataTable columns={columns} data={[]} emptyMessage="Empty catalog" />);
    expect(screen.getByText("Empty catalog")).toBeInTheDocument();
  });

  it("provides OpenScaleTransport via OpenScaleProvider and throws outside provider", () => {
    const mockTransport = {
      discoverScales: vi.fn(),
      listScales: vi.fn(),
      upsertScale: vi.fn(),
      listPlus: vi.fn(),
      getPlu: vi.fn(),
      upsertPlu: vi.fn(),
      deletePlu: vi.fn(),
      subscribe: vi.fn(() => () => undefined),
    } as unknown as OpenScaleTransport;

    render(
      <OpenScaleProvider transport={mockTransport}>
        <TransportConsumer />
      </OpenScaleProvider>,
    );
    expect(screen.getByTestId("has-transport")).toHaveTextContent("function");

    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    expect(() => render(<TransportConsumer />)).toThrow(
      "useOpenScaleTransport must be used within an <OpenScaleProvider>",
    );
    consoleSpy.mockRestore();
  });
});
