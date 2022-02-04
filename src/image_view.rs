use std::ops::Range;
use std::sync::Arc;
use vulkano::device::physical::FormatFeatures;
use vulkano::device::{Device, DeviceOwned};
use vulkano::format::Format;
use vulkano::image::immutable::SubImage;
use vulkano::image::view::{ImageView, ImageViewCreationError, ImageViewType};
use vulkano::image::{
    AttachmentImage, ImageAccess, ImageAspects, ImageDescriptorLayouts, ImageDimensions,
    ImageInner, ImageLayout, ImageUsage, ImageViewAbstract, ImmutableImage, StorageImage,
};
use vulkano::sampler::ycbcr::SamplerYcbcrConversion;
use vulkano::sampler::ComponentMapping;
use vulkano::sync::AccessError;
use vulkano::VulkanObject;

enum ImageVarient {
    Storage(Arc<StorageImage>),
    Immutable(Arc<ImmutableImage>),
    Sub(Arc<SubImage>),
    Attachment(Arc<AttachmentImage>),
}

unsafe impl ImageAccess for ImageVarient {
    #[inline]
    fn inner(&self) -> ImageInner<'_> {
        match self {
            Self::Storage(i) => i.inner(),
            Self::Immutable(i) => i.inner(),
            Self::Sub(i) => i.inner(),
            Self::Attachment(i) => i.inner(),
        }
    }

    #[inline]
    fn initial_layout_requirement(&self) -> ImageLayout {
        match self {
            Self::Storage(i) => i.initial_layout_requirement(),
            Self::Immutable(i) => i.initial_layout_requirement(),
            Self::Sub(i) => i.initial_layout_requirement(),
            Self::Attachment(i) => i.initial_layout_requirement(),
        }
    }

    #[inline]
    fn final_layout_requirement(&self) -> ImageLayout {
        match self {
            Self::Storage(i) => i.final_layout_requirement(),
            Self::Immutable(i) => i.final_layout_requirement(),
            Self::Sub(i) => i.final_layout_requirement(),
            Self::Attachment(i) => i.final_layout_requirement(),
        }
    }

    #[inline]
    fn descriptor_layouts(&self) -> Option<ImageDescriptorLayouts> {
        match self {
            Self::Storage(i) => i.descriptor_layouts(),
            Self::Immutable(i) => i.descriptor_layouts(),
            Self::Sub(i) => i.descriptor_layouts(),
            Self::Attachment(i) => i.descriptor_layouts(),
        }
    }

    #[inline]
    fn conflict_key(&self) -> u64 {
        match self {
            Self::Storage(i) => i.conflict_key(),
            Self::Immutable(i) => i.conflict_key(),
            Self::Sub(i) => i.conflict_key(),
            Self::Attachment(i) => i.conflict_key(),
        }
    }

    #[inline]
    fn current_mip_levels_access(&self) -> Range<u32> {
        match self {
            Self::Storage(i) => i.current_mip_levels_access(),
            Self::Immutable(i) => i.current_mip_levels_access(),
            Self::Sub(i) => i.current_mip_levels_access(),
            Self::Attachment(i) => i.current_mip_levels_access(),
        }
    }

    #[inline]
    fn current_array_layers_access(&self) -> Range<u32> {
        match self {
            Self::Storage(i) => i.current_array_layers_access(),
            Self::Immutable(i) => i.current_array_layers_access(),
            Self::Sub(i) => i.current_array_layers_access(),
            Self::Attachment(i) => i.current_array_layers_access(),
        }
    }

    #[inline]
    fn try_gpu_lock(
        &self,
        exclusive_access: bool,
        uninitialized_safe: bool,
        expected_layout: ImageLayout,
    ) -> Result<(), AccessError> {
        match self {
            Self::Storage(i) =>
                i.try_gpu_lock(exclusive_access, uninitialized_safe, expected_layout),
            Self::Immutable(i) =>
                i.try_gpu_lock(exclusive_access, uninitialized_safe, expected_layout),
            Self::Sub(i) =>
                i.try_gpu_lock(exclusive_access, uninitialized_safe, expected_layout),
            Self::Attachment(i) =>
                i.try_gpu_lock(exclusive_access, uninitialized_safe, expected_layout),
        }
    }

    #[inline]
    unsafe fn increase_gpu_lock(&self) {
        match self {
            Self::Storage(i) => i.increase_gpu_lock(),
            Self::Immutable(i) => i.increase_gpu_lock(),
            Self::Sub(i) => i.increase_gpu_lock(),
            Self::Attachment(i) => i.increase_gpu_lock(),
        }
    }

    #[inline]
    unsafe fn unlock(&self, transitioned_layout: Option<ImageLayout>) {
        match self {
            Self::Storage(i) => i.unlock(transitioned_layout),
            Self::Immutable(i) => i.unlock(transitioned_layout),
            Self::Sub(i) => i.unlock(transitioned_layout),
            Self::Attachment(i) => i.unlock(transitioned_layout),
        }
    }
}

pub struct ImtImageView {
    view: Arc<ImageView<ImageVarient>>,
}

impl ImtImageView {
    pub fn from_storage(image: Arc<StorageImage>) -> Result<Arc<Self>, ImageViewCreationError> {
        Ok(Arc::new(ImtImageView {
            view: ImageView::new(Arc::new(ImageVarient::Storage(image)))?,
        }))
    }

    pub fn from_immutable(
        image: Arc<ImmutableImage>,
    ) -> Result<Arc<Self>, ImageViewCreationError> {
        Ok(Arc::new(ImtImageView {
            view: ImageView::new(Arc::new(ImageVarient::Immutable(image)))?,
        }))
    }

    pub fn from_sub(image: Arc<SubImage>) -> Result<Arc<Self>, ImageViewCreationError> {
        Ok(Arc::new(ImtImageView {
            view: ImageView::new(Arc::new(ImageVarient::Sub(image)))?,
        }))
    }

    pub fn from_attachment(
        image: Arc<AttachmentImage>,
    ) -> Result<Arc<Self>, ImageViewCreationError> {
        Ok(Arc::new(ImtImageView {
            view: ImageView::new(Arc::new(ImageVarient::Attachment(image)))?,
        }))
    }

    #[inline]
    pub fn dimensions(&self) -> ImageDimensions {
        self.view.image().dimensions()
    }
}

unsafe impl ImageViewAbstract for ImtImageView {
    #[inline]
    fn image(&self) -> Arc<dyn ImageAccess> {
        self.view.image().clone() as Arc<dyn ImageAccess>
    }

    #[inline]
    fn array_layers(&self) -> Range<u32> {
        self.view.array_layers()
    }

    #[inline]
    fn aspects(&self) -> &ImageAspects {
        self.view.aspects()
    }

    #[inline]
    fn component_mapping(&self) -> ComponentMapping {
        self.view.component_mapping()
    }

    #[inline]
    fn filter_cubic(&self) -> bool {
        self.view.filter_cubic()
    }

    #[inline]
    fn filter_cubic_minmax(&self) -> bool {
        self.view.filter_cubic_minmax()
    }

    #[inline]
    fn format(&self) -> Format {
        self.view.format()
    }

    #[inline]
    fn format_features(&self) -> &FormatFeatures {
        self.view.format_features()
    }

    #[inline]
    fn mip_levels(&self) -> Range<u32> {
        self.view.mip_levels()
    }

    #[inline]
    fn sampler_ycbcr_conversion(&self) -> Option<&Arc<SamplerYcbcrConversion>> {
        self.view.sampler_ycbcr_conversion()
    }

    #[inline]
    fn ty(&self) -> ImageViewType {
        self.view.ty()
    }

    #[inline]
    fn usage(&self) -> &ImageUsage {
        self.view.usage()
    }
}

unsafe impl VulkanObject for ImtImageView {
    type Object = ash::vk::ImageView;

    #[inline]
    fn internal_object(&self) -> ash::vk::ImageView {
        self.view.internal_object()
    }
}

unsafe impl DeviceOwned for ImtImageView {
    fn device(&self) -> &Arc<Device> {
        self.view.device()
    }
}

unsafe impl ImageAccess for ImtImageView {
    #[inline]
    fn inner(&self) -> ImageInner<'_> {
        self.view.image().inner()
    }

    #[inline]
    fn initial_layout_requirement(&self) -> ImageLayout {
        self.view.image().initial_layout_requirement()
    }

    #[inline]
    fn final_layout_requirement(&self) -> ImageLayout {
        self.view.image().final_layout_requirement()
    }

    #[inline]
    fn descriptor_layouts(&self) -> Option<ImageDescriptorLayouts> {
        self.view.image().descriptor_layouts()
    }

    #[inline]
    fn conflict_key(&self) -> u64 {
        self.view.image().conflict_key()
    }

    #[inline]
    fn current_mip_levels_access(&self) -> Range<u32> {
        self.view.image().current_mip_levels_access()
    }

    #[inline]
    fn current_array_layers_access(&self) -> Range<u32> {
        self.view.image().current_array_layers_access()
    }

    #[inline]
    fn try_gpu_lock(
        &self,
        exclusive_access: bool,
        uninitialized_safe: bool,
        expected_layout: ImageLayout,
    ) -> Result<(), AccessError> {
        self.view.image().try_gpu_lock(exclusive_access, uninitialized_safe, expected_layout)
    }

    #[inline]
    unsafe fn increase_gpu_lock(&self) {
        self.view.image().increase_gpu_lock()
    }

    #[inline]
    unsafe fn unlock(&self, transitioned_layout: Option<ImageLayout>) {
        self.view.image().unlock(transitioned_layout)
    }
}
