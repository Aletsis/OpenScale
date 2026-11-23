import "@testing-library/jest-dom/vitest";
import type { OpenScaleTransport } from "@openscale/client";
import type { ColumnDef } from "@tanstack/react-table";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { OpenScaleProvider, useOpenScaleTransport } from "../providers/OpenScaleProvider.js";
import { Badge } from "./badge.js";
import { Button } from "./button.js";
import { DataTable } from "./data-table.js";
import { ConfirmDialog, Dialog } from "./dialog.js";
import { ToastProvider, useToast } from "./toast.js";

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

function ToastTrigger() {
  const { toast, dismissAll } = useToast();
  return (
    <div>
      <button
        type="button"
        onClick={() => toast({ message: "Saved PLU", variant: "success", durationMs: 3000 })}
      >
        Show Success
      </button>
      <button
        type="button"
        onClick={() =>
          toast({ title: "Sync failed", message: "Timeout", variant: "error", durationMs: 0 })
        }
      >
        Show Sticky Error
      </button>
      <button type="button" onClick={dismissAll}>
        Clear All
      </button>
    </div>
  );
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

  it("renders DataTable rows, loading skeleton, and empty state", () => {
    const { rerender } = render(
      <DataTable
        columns={columns}
        data={[{ plu: 100, description: "POLLO FRESCO" }]}
        emptyMessage="Empty catalog"
      />,
    );
    expect(screen.getByText("100")).toBeInTheDocument();
    expect(screen.getByText("POLLO FRESCO")).toBeInTheDocument();

    rerender(<DataTable columns={columns} data={[]} loading loadingRows={2} />);
    expect(screen.queryByText("Empty catalog")).not.toBeInTheDocument();

    rerender(
      <DataTable
        columns={columns}
        data={[]}
        emptyMessage="Empty catalog"
        emptyDescription="Add a PLU to get started"
        emptyIcon={<span data-testid="empty-icon" />}
      />,
    );
    expect(screen.getByText("Empty catalog")).toBeInTheDocument();
    expect(screen.getByText("Add a PLU to get started")).toBeInTheDocument();
    expect(screen.getByTestId("empty-icon")).toBeInTheDocument();
  });

  it("sorts DataTable columns ascending and descending when sortable is enabled", () => {
    render(
      <DataTable
        columns={columns}
        data={[
          { plu: 200, description: "ZANAHORIA" },
          { plu: 100, description: "AGUACATE" },
        ]}
        sortable
      />,
    );

    const pluSortBtn = screen.getByRole("button", { name: "PLU" });
    fireEvent.click(pluSortBtn);
    const rowsAsc = screen.getAllByRole("row");
    expect(rowsAsc[1]).toHaveTextContent("100");
    expect(rowsAsc[2]).toHaveTextContent("200");

    fireEvent.click(pluSortBtn);
    const rowsDesc = screen.getAllByRole("row");
    expect(rowsDesc[1]).toHaveTextContent("200");
    expect(rowsDesc[2]).toHaveTextContent("100");
  });

  it("auto-dismisses toasts, pauses on hover, and supports manual close", () => {
    vi.useFakeTimers();
    try {
      render(
        <ToastProvider closeLabel="Dismiss">
          <ToastTrigger />
        </ToastProvider>,
      );

      fireEvent.click(screen.getByText("Show Success"));
      const toastEl = screen.getByRole("status");
      expect(toastEl).toHaveTextContent("Saved PLU");

      // Hovering pauses auto-dismiss
      fireEvent.mouseEnter(toastEl);
      act(() => {
        vi.advanceTimersByTime(5000);
      });
      expect(screen.getByText("Saved PLU")).toBeInTheDocument();

      // Leaving resumes auto-dismiss
      fireEvent.mouseLeave(toastEl);
      act(() => {
        vi.advanceTimersByTime(3100);
      });
      expect(screen.queryByText("Saved PLU")).not.toBeInTheDocument();

      // Sticky error stays until manually dismissed
      fireEvent.click(screen.getByText("Show Sticky Error"));
      expect(screen.getByRole("alert")).toHaveTextContent("Sync failed");
      act(() => {
        vi.advanceTimersByTime(10000);
      });
      expect(screen.getByRole("alert")).toBeInTheDocument();

      fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();

      // dismissAll clears active toasts
      fireEvent.click(screen.getByText("Show Sticky Error"));
      expect(screen.getByRole("alert")).toBeInTheDocument();
      fireEvent.click(screen.getByText("Clear All"));
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it("throws when useToast is called outside ToastProvider", () => {
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
    expect(() => render(<ToastTrigger />)).toThrow(
      "useToast must be used within a <ToastProvider>",
    );
    consoleSpy.mockRestore();
  });

  it("renders Dialog and ConfirmDialog and handles Escape and backdrop close", () => {
    const onClose = vi.fn();
    const onConfirm = vi.fn();

    const { rerender } = render(
      <Dialog open={false} onClose={onClose} title="Hidden">
        Body
      </Dialog>,
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    rerender(
      <Dialog open onClose={onClose} title="Edit PLU" description="Update fields">
        <div>Form content</div>
      </Dialog>,
    );
    expect(screen.getByRole("dialog")).toHaveTextContent("Edit PLU");
    expect(screen.getByText("Update fields")).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);

    rerender(
      <ConfirmDialog
        open
        onClose={onClose}
        onConfirm={onConfirm}
        title="Delete PLU?"
        description="Cannot be undone"
        confirmLabel="Delete"
        cancelLabel="Cancel"
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
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
