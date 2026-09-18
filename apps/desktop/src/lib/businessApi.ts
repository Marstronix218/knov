import { invoke } from "@tauri-apps/api/core";
import type { BusinessAction, BusinessWorkspace, ExportArtifact } from "../businessTypes";
import { mockMutate, mockPreviewExport, mockSaveExport, mockWorkspace } from "./businessMock";

const isTauri = () => "__TAURI_INTERNALS__" in window;

export const businessApi = {
  workspace: (): Promise<BusinessWorkspace> => isTauri() ? invoke("business_workspace") : mockWorkspace(),
  mutate: (request: BusinessAction): Promise<BusinessWorkspace> => isTauri() ? invoke("business_action", { request }) : mockMutate(request),
  previewExport: (certificationId: string, format: "csv" | "json"): Promise<ExportArtifact> => isTauri() ? invoke("preview_business_export", { certificationId, format }) : mockPreviewExport(certificationId, format),
  saveExport: async (artifact: ExportArtifact): Promise<string | null> => {
    if (isTauri()) return invoke("save_business_export", { certificationId: artifact.certificationId, format: artifact.format, expectedContent: artifact.content });
    const result = await mockSaveExport(artifact);
    const blob = new Blob([artifact.content], { type: artifact.format === "json" ? "application/json" : "text/csv" });
    if (URL.createObjectURL) {
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url; link.download = artifact.fileName; link.click();
      URL.revokeObjectURL(url);
    }
    return result;
  },
};
