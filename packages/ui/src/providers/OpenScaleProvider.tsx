import type { OpenScaleTransport } from "@openscale/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { type ReactNode, createContext, useContext, useMemo } from "react";

const TransportContext = createContext<OpenScaleTransport | null>(null);

export interface OpenScaleProviderProps {
  transport: OpenScaleTransport;
  queryClient?: QueryClient;
  children: ReactNode;
}

export function OpenScaleProvider({ transport, queryClient, children }: OpenScaleProviderProps) {
  const client = useMemo(
    () =>
      queryClient ??
      new QueryClient({
        defaultOptions: {
          queries: {
            staleTime: 10_000,
            retry: 1,
          },
        },
      }),
    [queryClient],
  );

  return (
    <TransportContext.Provider value={transport}>
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    </TransportContext.Provider>
  );
}

export function useOpenScaleTransport(): OpenScaleTransport {
  const transport = useContext(TransportContext);
  if (!transport) {
    throw new Error("useOpenScaleTransport must be used within an <OpenScaleProvider>");
  }
  return transport;
}
