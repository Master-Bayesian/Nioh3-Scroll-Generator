import React, { useEffect, useReducer, useRef } from "react";
import { LoadoutOptimizer, loadoutDiscovery } from "./LoadoutOptimizer";
import { LoadoutDiscoveryPanel } from "./LoadoutDiscoveryPanel";
import { discoverSaves } from "./save-workspace";
import { initialLoadoutDiscoveryState, loadoutDiscoveryReducer } from "./loadout-discovery-state";

type Discovery = Awaited<ReturnType<typeof loadoutDiscovery>>;

export function LoadoutPage() {
  const [state, dispatch] = useReducer(loadoutDiscoveryReducer<Discovery>, initialLoadoutDiscoveryState<Discovery>());
  const serial = useRef(0);
  function read() {
    const requestId = ++serial.current;
    dispatch({ type: "Requested", requestId });
    void discoverSaves().then(loadoutDiscovery).then(
      (data) => dispatch({ type: "Resolved", requestId, data }),
      (error: unknown) => dispatch({ type: "Failed", requestId, message: String(error) }),
    );
  }
  useEffect(read, []);
  return state.kind === "Ready"
    ? <LoadoutOptimizer bundles={state.data.bundles} catalog={state.data.catalog} />
    : <LoadoutDiscoveryPanel state={state} retry={read} />;
}
