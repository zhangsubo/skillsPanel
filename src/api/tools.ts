import { invokeCommand } from './index';
import type { Tool } from '@/types';

export async function getTools(): Promise<Tool[]> {
  return invokeCommand<Tool[]>('get_tools');
}