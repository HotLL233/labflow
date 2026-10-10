import type { WorkloadPerson, WorkloadMatrixRow } from '../api/workloadReport';

// Stable IDs distinguish same-name accounts; unbound history remains a separate row.
export const workloadPersonKey = (person: { user_id: number | null; user_name: string }): string =>
  person.user_id === null ? `legacy:${person.user_name}` : `id:${person.user_id}`;

export function buildWorkloadMatrix(people: WorkloadPerson[], matrix: WorkloadMatrixRow[]) {
  const types = [...new Set(matrix.map(row => row.type))].sort((a, b) => a.localeCompare(b, 'zh-CN'));
  const columns = new Map(types.map(type => [type, 0]));
  const cells = new Map<string, Map<string, number>>();
  for (const row of matrix) {
    const key = workloadPersonKey(row);
    const person = cells.get(key) ?? new Map<string, number>();
    person.set(row.type, (person.get(row.type) ?? 0) + row.total_workload);
    cells.set(key, person);
    columns.set(row.type, (columns.get(row.type) ?? 0) + row.total_workload);
  }
  return {
    types, columns,
    rows: people.map(person => ({ ...person, key: workloadPersonKey(person), cells: cells.get(workloadPersonKey(person)) ?? new Map<string, number>() })),
    total: people.reduce((sum, person) => sum + person.total_workload, 0),
  };
}
