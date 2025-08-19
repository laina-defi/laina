import { describe, expect, it } from 'vitest';
import { parseErrorMessage } from './error-parsing';

describe('ErrorDialog parseErrorMessage', () => {
  const exampleErrorMessage = `Transaction simulation failed: "HostError: Error(WasmVm, InvalidAction)"

Event log (newest first):
   0: [Diagnostic Event] contract:MOCK_CONTRACT_A, topics:[error, Error(WasmVm, InvalidAction)], data:"escalating error to VM trap from failed host function call: call"
   1: [Diagnostic Event] contract:MOCK_CONTRACT_A, topics:[error, Error(WasmVm, InvalidAction)], data:["contract call failed", repay, [MOCK_ADDRESS, 2368612289, 137939]]
   2: [Failed Diagnostic Event (not emitted)] contract:MOCK_CONTRACT_B, topics:[error, Error(WasmVm, InvalidAction)], data:["VM call trapped: UnreachableCodeReached", repay]
   3: [Failed Diagnostic Event (not emitted)] contract:MOCK_CONTRACT_C, topics:[fn_return, transfer], data:Void
   4: [Failed Contract Event (not emitted)] contract:MOCK_CONTRACT_C, topics:[transfer, MOCK_ADDRESS, MOCK_CONTRACT_A, "native"], data:13793
   5: [Failed Diagnostic Event (not emitted)] contract:MOCK_CONTRACT_B, topics:[fn_call, MOCK_CONTRACT_C, transfer], data:[MOCK_ADDRESS, MOCK_CONTRACT_A, 13793]
   6: [Failed Diagnostic Event (not emitted)] contract:MOCK_CONTRACT_C, topics:[fn_return, transfer], data:Void
   7: [Failed Contract Event (not emitted)] contract:MOCK_CONTRACT_C, topics:[transfer, MOCK_ADDRESS, MOCK_CONTRACT_B, "native"], data:2368598496
   8: [Failed Diagnostic Event (not emitted)] contract:MOCK_CONTRACT_B, topics:[fn_call, MOCK_CONTRACT_C, transfer], data:[MOCK_ADDRESS, MOCK_CONTRACT_B, 2368598496]
   9: [Failed Contract Event (not emitted)] contract:MOCK_CONTRACT_B, topics:[[Accrual], "updated"], data:10000260
   10: [Failed Contract Event (not emitted)] contract:MOCK_CONTRACT_B, topics:[[AccrualLastUpdate], "updated"], data:1755604270
   11: [Diagnostic Event] contract:MOCK_CONTRACT_A, topics:[fn_call, MOCK_CONTRACT_B, repay], data:[MOCK_ADDRESS, 2368612289, 137939]
   12: [Contract Event] contract:MOCK_CONTRACT_A, topics:[Loan, updated], data:[Loan, {borrower_address: MOCK_ADDRESS, nonce: 3}]
   13: [Diagnostic Event] contract:MOCK_CONTRACT_D, topics:[fn_return, twap], data:40872325555966
   14: [Diagnostic Event] contract:MOCK_CONTRACT_A, topics:[fn_call, MOCK_CONTRACT_D, twap], data:[[Other, XLM], 12]
   15: [Diagnostic Event] contract:MOCK_CONTRACT_D, topics:[fn_return, twap], data:99966371318567
   16: [Diagnostic Event] contract:MOCK_CONTRACT_A, topics:[fn_call, MOCK_CONTRACT_D, twap], data:[[Other, USDC], 12]
   17: [Diagnostic Event] contract:MOCK_CONTRACT_E, topics:[fn_return, get_collateral_factor], data:8000000
   18: [Diagnostic Event] contract:MOCK_CONTRACT_A, topics:[fn_call, MOCK_CONTRACT_E, get_collateral_factor], data:Void
   19: [Diagnostic Event] contract:MOCK_CONTRACT_B, topics:[fn_return, get_accrual], data:10000260
   20: [Diagnostic Event] contract:MOCK_CONTRACT_A, topics:[fn_call, MOCK_CONTRACT_B, get_accrual], data:Void
   21: [Diagnostic Event] contract:MOCK_CONTRACT_B, topics:[fn_return, add_interest_to_accrual], data:Void
   22: [Contract Event] contract:MOCK_CONTRACT_B, topics:[[Accrual], "updated"], data:10000260
   23: [Contract Event] contract:MOCK_CONTRACT_B, topics:[[AccrualLastUpdate], "updated"], data:1755604270
   24: [Diagnostic Event] contract:MOCK_CONTRACT_A, topics:[fn_call, MOCK_CONTRACT_B, add_interest_to_accrual], data:Void
   25: [Diagnostic Event] contract:MOCK_CONTRACT_E, topics:[fn_return, get_currency], data:{ticker: USDC, token_address: MOCK_TOKEN_USDC}
   26: [Diagnostic Event] contract:MOCK_CONTRACT_A, topics:[fn_call, MOCK_CONTRACT_E, get_currency], data:Void
   27: [Diagnostic Event] contract:MOCK_CONTRACT_B, topics:[fn_return, get_currency], data:{ticker: XLM, token_address: MOCK_TOKEN_XLM}
   28: [Diagnostic Event] contract:MOCK_CONTRACT_A, topics:[fn_call, MOCK_CONTRACT_B, get_currency], data:Void
   29: [Diagnostic Event] topics:[fn_call, MOCK_CONTRACT_A, repay], data:[{borrower_address: MOCK_ADDRESS, nonce: 3}, 2368612289]`;

  it('should parse the main error correctly', () => {
    const result = parseErrorMessage(exampleErrorMessage);

    expect(result.mainError).toBe('Transaction simulation failed: "HostError: Error(WasmVm, InvalidAction)"');
  });

  it('should parse all event log entries', () => {
    const result = parseErrorMessage(exampleErrorMessage);

    expect(result.eventLog).toHaveLength(30); // 0-29 = 30 entries
  });

  it('should parse event log entries with correct structure', () => {
    const result = parseErrorMessage(exampleErrorMessage);

    // Test first entry
    const firstEntry = result.eventLog[0];
    expect(firstEntry).toEqual({
      index: 0,
      type: 'Diagnostic Event',
      contract: 'MOCK_CONTRACT_A',
      topics: ['error', 'Error(WasmVm', 'InvalidAction)'],
      data: '"escalating error to VM trap from failed host function call: call"',
    });

    // Test an entry without contract
    const entryWithoutContract = result.eventLog[29];
    expect(entryWithoutContract).toEqual({
      index: 29,
      type: 'Diagnostic Event',
      contract: undefined,
      topics: ['fn_call', 'MOCK_CONTRACT_A', 'repay'],
      data: '[{borrower_address: MOCK_ADDRESS, nonce: 3}, 2368612289]',
    });
  });

  it('should handle different event types correctly', () => {
    const result = parseErrorMessage(exampleErrorMessage);

    const eventTypes = result.eventLog.map((entry) => entry.type);

    expect(eventTypes).toContain('Diagnostic Event');
    expect(eventTypes).toContain('Failed Diagnostic Event (not emitted)');
    expect(eventTypes).toContain('Failed Contract Event (not emitted)');
    expect(eventTypes).toContain('Contract Event');
  });

  it('should parse topics correctly', () => {
    const result = parseErrorMessage(exampleErrorMessage);

    // Test complex topics with brackets
    const entryWithComplexTopics = result.eventLog.find((entry) => entry.topics.some((topic) => topic.includes('[')));

    expect(entryWithComplexTopics).toBeDefined();
    expect(entryWithComplexTopics?.topics).toContain('[Accrual]');
    expect(entryWithComplexTopics?.topics).toContain('"updated"');
  });

  it('should handle empty or malformed messages gracefully', () => {
    const emptyResult = parseErrorMessage('');
    expect(emptyResult.mainError).toBe('');
    expect(emptyResult.eventLog).toHaveLength(0);

    const singleLineResult = parseErrorMessage('Just a simple error message');
    expect(singleLineResult.mainError).toBe('Just a simple error message');
    expect(singleLineResult.eventLog).toHaveLength(0);
  });

  it('should identify entries that have contracts', () => {
    const result = parseErrorMessage(exampleErrorMessage);

    const entriesWithContracts = result.eventLog.filter((entry) => entry.contract);

    expect(entriesWithContracts.length).toBeGreaterThan(0);
    expect(entriesWithContracts[0]?.contract).toMatch(/^MOCK_CONTRACT_/);
  });

  it('should maintain correct order of event log entries', () => {
    const result = parseErrorMessage(exampleErrorMessage);

    // Check that indices are in order
    for (let i = 0; i < result.eventLog.length - 1; i++) {
      expect(result.eventLog[i]?.index).toBeLessThan(result.eventLog[i + 1]?.index ?? 0);
    }
  });

  it('should handle data with complex structures', () => {
    const result = parseErrorMessage(exampleErrorMessage);

    // Find entry with complex data structure
    const complexDataEntry = result.eventLog.find(
      (entry) => entry.data.includes('{borrower_address:') && entry.data.includes('nonce:'),
    );

    expect(complexDataEntry).toBeDefined();
    expect(complexDataEntry?.data).toContain('borrower_address: MOCK_ADDRESS');
    expect(complexDataEntry?.data).toContain('nonce: 3');
  });
});
