// Dynamic currency loader for frontend
export const loadCurrencies = async () => {
  const isLocal = import.meta.env.STELLAR_NETWORK === 'local';

  if (isLocal) {
    return await import('../../currencies-local');
  }
  return await import('../../currencies');
};
