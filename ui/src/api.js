import Urbit from '@urbit/http-api';

let api = null;

export async function getApi() {
  if (api) return api;
  api = new Urbit('');
  api.ship = window.ship;
  await api.connect();
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
