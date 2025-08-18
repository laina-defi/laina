// Dynamic currency loader for frontend
export const loadCurrencies = async () => {
  return await import('../../currencies');
};
