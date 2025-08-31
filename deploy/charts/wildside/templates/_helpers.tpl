{{- define "wildside.name" -}}
{{- default .Chart.Name .Values.nameOverride | lower | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "wildside.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- $name := include "wildside.name" . -}}
{{- if eq .Release.Name $name -}}
{{- $name | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{- define "wildside.labels" -}}
app.kubernetes.io/name: {{ include "wildside.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" }}
{{- end -}}

{{- define "wildside.selectorLabels" -}}
app.kubernetes.io/name: {{ include "wildside.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{/*
Validate that secretEnvFromKeys references an existing Secret when set.

- Requires .Values.existingSecretName when secretEnvFromKeys has entries.
- Optionally fails if the Secret is missing and allowMissingSecret is false.
*/}}
{{- define "wildside.validateSecrets" -}}
{{- $sec := .Values.secretEnvFromKeys | default dict -}}
{{- $name := .Values.existingSecretName -}}
{{- $allowMissing := .Values.allowMissingSecret | default true -}}
{{- if and (gt (len $sec) 0) (not $name) -}}
{{- fail "existingSecretName is required when secretEnvFromKeys is set" -}}
{{- end -}}
{{- if and (gt (len $sec) 0) $name -}}
{{- $found := lookup "v1" "Secret" .Release.Namespace $name -}}
{{- if and (not $found) (not $allowMissing) -}}
{{- fail (printf "Secret %q not found in namespace %q" $name .Release.Namespace) -}}
{{- end -}}
{{- end -}}
{{- end -}}
