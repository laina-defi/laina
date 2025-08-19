export interface EventLogEntry {
  index: number;
  type: EventType;
  contract: string | undefined;
  topics: string[];
  data: string;
}

export type EventType =
  | 'Diagnostic Event'
  | 'Failed Diagnostic Event (not emitted)'
  | 'Failed Contract Event (not emitted)'
  | 'Contract Event';

export interface ParsedError {
  mainError: string;
  eventLog: EventLogEntry[];
}

export function parseErrorMessage(message: string): ParsedError {
  const lines = message.split('\n');
  const mainError = lines[0] || '';

  const eventLog: EventLogEntry[] = [];
  let currentEntry: Partial<EventLogEntry> = {};

  for (let i = 1; i < lines.length; i++) {
    const line = lines[i]?.trim() || '';

    // Skip empty lines and the "Event log (newest first):" header
    if (!line || line === 'Event log (newest first):') {
      continue;
    }

    // Check if this is an event log entry (starts with a number)
    const eventMatch = line.match(
      /^\s*(\d+)\s*:\s*\[([^\]]+)\]\s*(?:contract:([^\s,]+),\s*)?topics:\[(.+?)\],\s*data:(.+)$/,
    );

    if (eventMatch?.[1] && eventMatch?.[2] && eventMatch?.[4] && eventMatch?.[5]) {
      // Save previous entry if exists
      if (currentEntry.index !== undefined) {
        eventLog.push(currentEntry as EventLogEntry);
      }

      // Parse topics: split by comma but respect nested brackets
      const topicsStr = eventMatch[4];
      const topics: string[] = [];
      let currentTopic = '';
      let bracketCount = 0;

      for (let j = 0; j < topicsStr.length; j++) {
        const char = topicsStr[j];
        if (char === '[') bracketCount++;
        if (char === ']') bracketCount--;

        if (char === ',' && bracketCount === 0) {
          topics.push(currentTopic.trim());
          currentTopic = '';
        } else {
          currentTopic += char;
        }
      }
      if (currentTopic) {
        topics.push(currentTopic.trim());
      }

      // Start new entry
      currentEntry = {
        index: Number.parseInt(eventMatch[1]),
        type: eventMatch[2] as EventLogEntry['type'],
        contract: eventMatch[3] || undefined,
        topics,
        data: eventMatch[5].trim(),
      };
    }
  }

  // Add the last entry
  if (currentEntry.index !== undefined) {
    eventLog.push(currentEntry as EventLogEntry);
  }

  return {
    mainError,
    eventLog,
  };
}
