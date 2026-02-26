import Urbit from '@urbit/http-api';

let api = null;

// In dev mode (Vite), we proxy to localhost:8080 (fakezod).
// In production (served from Urbit), auth is already handled.
const DEV_CODE = 'lidlut-tabwed-pillex-ridrup';
const isDev = import.meta.env.DEV;

export async function getApi() {
  if (api) return api;

  if (isDev) {
    // Use the library's built-in auth flow
    api = await Urbit.authenticate({
      ship: 'zod',
      url: '',
      code: DEV_CODE,
      verbose: false,
    });
  } else {
    // Production: served from Urbit, already authenticated
    api = new Urbit('');
    api.ship = window.ship;
    await api.eventSource();
  }

  return api;
}

export async function scry(path) {
  const urb = await getApi();
  return urb.scry({ app: 'lora-agent', path });
}

export async function poke(json) {
  const urb = await getApi();
  return urb.poke({
    app: 'lora-agent',
    mark: 'json',
    json,
  });
}

export async function subscribe(path, handler) {
  const urb = await getApi();
  return urb.subscribe({
    app: 'lora-agent',
    path,
    event: handler,
    err: (err) => console.error('Subscription error:', path, err),
    quit: () => console.warn('Subscription quit:', path),
  });
}
