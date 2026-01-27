#!/bin/bash

# sync_to_main.sh
# Creates a new PR to sync codecrafters branch to main (excluding CC files)
# Usage: ./sync_to_main.sh [branch-name]

set -e  # Exit on error

# Load shared functions
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/sync_common.sh"

CURRENT_BRANCH=$(git branch --show-current)

echo -e "${BLUE}🚀 Creating new PR from ${CODECRAFTERS_BRANCH} to ${MAIN_BRANCH}${NC}"
echo ""

# Get custom name from argument or prompt
if [[ -n "$1" ]]; then
    CUSTOM_NAME="$1"
    echo -e "${BLUE}Using branch name:${NC} ${CUSTOM_NAME}"
else
    echo -e "${YELLOW}Enter a name for this sync (e.g., 'add-grep', 'implement-pipes'):${NC}"
    read -p "> " CUSTOM_NAME
fi

# Validate input
if [[ -z "$CUSTOM_NAME" ]]; then
    echo -e "${RED}❌ Branch name cannot be empty${NC}"
    exit 1
fi

SYNC_BRANCH=$(sanitize_branch_name "$CUSTOM_NAME")

echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

check_clean_state
update_codecrafters

# Check if branch already exists
if git show-ref --verify --quiet "refs/heads/${SYNC_BRANCH}"; then
    echo -e "${RED}❌ Branch '${SYNC_BRANCH}' already exists locally${NC}"
    echo -e "${YELLOW}Please delete it first, choose a different name, or use:${NC}"
    echo -e "   ${YELLOW}./sync_pr.sh ${SYNC_BRANCH}${NC}"
    exit 1
fi

create_sync_branch "$SYNC_BRANCH"
PR_URL=$(generate_pr_url "$SYNC_BRANCH")

echo ""
echo -e "${GREEN}✅ Sync branch created and pushed successfully!${NC}"

display_pr_info "$SYNC_BRANCH" "$PR_URL"
open_in_browser "$PR_URL"

# Return to original branch
echo -e "${BLUE}🔄 Returning to ${CURRENT_BRANCH} branch...${NC}"
git checkout "$CURRENT_BRANCH"

echo ""
echo -e "${GREEN}✨ Sync complete!${NC}"
echo ""
echo -e "${BLUE}Next steps:${NC}"
echo -e "  1. ${YELLOW}Click the URL above${NC} (or copy/paste it into your browser)"
echo -e "  2. Title will be pre-filled: ${GREEN}$(generate_pr_title "$SYNC_BRANCH")${NC}"
echo -e "  3. Review the changes and add a description"
echo -e "  4. Click ${GREEN}'Create pull request'${NC}"
echo -e "  5. Merge when ready"
echo ""
echo -e "${BLUE}To update this PR later with changes:${NC}"
echo -e "  ${YELLOW}./sync_pr.sh ${SYNC_BRANCH}${NC}"
echo ""
