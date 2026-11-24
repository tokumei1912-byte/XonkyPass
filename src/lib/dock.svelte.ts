// Панель у края экрана: закреплена ли и в каком режиме. Источник — Rust (`dock://state`).

import { reportError } from './actions';
import { api, type DockState } from './api';

class DockStore {
  state = $state<DockState>({ visible: true, pinned: false, mode: 'left' });

  /** У края экрана (а не обычным окном): только тогда есть смысл закреплять. */
  get docked(): boolean {
    return this.state.mode !== 'window';
  }

  /** Подписка на состояние; возвращает отписку. */
  watch(): () => void {
    api.dockState().then(
      (state) => (this.state = state),
      () => {},
    );
    const unsubscribe = api.onDockState((state) => (this.state = state));
    return () => void unsubscribe.then((off) => off());
  }

  async togglePin(): Promise<void> {
    try {
      this.state = await api.dockPin(!this.state.pinned);
    } catch (error) {
      reportError(error);
    }
  }
}

export const dock = new DockStore();
