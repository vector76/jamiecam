import { typedInvoke } from './errors'
import type { MaterialMeta, FeedEntry } from './types'

export async function listMaterials(): Promise<MaterialMeta[]> {
  return typedInvoke<MaterialMeta[]>('list_materials')
}

export async function lookupFeeds(
  materialId: string,
  toolMaterial: string,
  operationCategory: string,
): Promise<FeedEntry> {
  return typedInvoke<FeedEntry>('lookup_feeds', { materialId, toolMaterial, operationCategory })
}
